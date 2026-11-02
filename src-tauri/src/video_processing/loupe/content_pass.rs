//! Magnified content, curved-rim refraction and directional outer shadow.
//! Includes spectral fringe sampling and filtered curved-rim reads.
//! Supports glass lighting and separate foreground/backdrop sampling.
use anyhow::{bail, Result};
use super::mip_pyramid::{MipPyramid, SAMPLING_SHADER};
use std::sync::OnceLock;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    center: [f32; 2],
    radius: f32,
    zoom: f32,
    presence: f32,
    bevel: f32,
    shadow_alpha: f32,
    shadow_reach: f32,
    light_direction: [f32; 2],
    light_tilt: f32,
    aberration: f32,
    reflection: f32,
    source_pixel_scale: f32,
    lighting: f32,
    shine_exponent: f32,
    separate_layers: f32,
    padding: [f32; 3],
}

/// Stage-pixel geometry, after crop and camera projection.
#[derive(Clone, Copy)]
pub struct ContentLens {
    pub center: [f32; 2],
    pub radius: f32,
    pub zoom: f32,
    pub presence: f32,
    /// Width of the curved rim in stage pixels; zero gives flat magnification.
    pub bevel: f32,
    pub shadow_alpha: f32,
    pub shadow_reach: f32,
    pub light_direction: [f32; 2],
    pub light_tilt: f32,
    pub aberration: f32,
    pub reflection: f32,
    /// Stage pixels per original source pixel, including crop/camera scaling.
    pub source_pixel_scale: f32,
    pub lighting: f32,
    pub shine_exponent: f32,
}

impl Default for ContentLens {
    fn default() -> Self {
        Self {center:[0.0;2],radius:0.0,zoom:1.0,presence:0.0,bevel:0.0,
            shadow_alpha:0.0,shadow_reach:0.0,light_direction:[0.0,-1.0],light_tilt:0.0,
            aberration:0.0,reflection:0.0,source_pixel_scale:1.0,lighting:0.0,shine_exponent:8.0}
    }
}

pub struct ContentPass {
    pipeline: wgpu::ComputePipeline,
    binding: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    output: wgpu::Texture,
    size: (u32, u32),
    device: wgpu::Device,
    input: wgpu::Texture,
    foreground: Option<wgpu::Texture>,
    background: Option<wgpu::Texture>,
    straight_output: bool,
    sampler: wgpu::Sampler,
    filtered: OnceLock<Result<(MipPyramid, wgpu::BindGroup), String>>,
}

impl ContentPass {
    /// Input already includes the screen content and its pointer/effects. Keep
    /// output separate so neighboring pixels never read partially written data.
    pub fn new(device: &wgpu::Device, input: &wgpu::Texture, size: (u32, u32)) -> Result<Self> {
        Self::with_layers(device,input,size,None)
    }

    /// All three images use premultiplied RGBA. Foreground contains screen,
    /// pointer and screen shadows; background contains the unobscured backdrop.
    pub fn new_layered(device:&wgpu::Device, composed:&wgpu::Texture,
        foreground:&wgpu::Texture, background:&wgpu::Texture, size:(u32,u32)) -> Result<Self> {
        Self::with_layers(device,composed,size,Some((foreground,background)))
    }

    fn with_layers(device:&wgpu::Device,input:&wgpu::Texture,size:(u32,u32),
        layers:Option<(&wgpu::Texture,&wgpu::Texture)>) -> Result<Self> {
        if size.0 == 0 || size.1 == 0 || input.width() != size.0 || input.height() != size.1 {
            bail!("Magnifier input dimensions do not match the stage");
        }
        for texture in [Some(input),layers.map(|p|p.0),layers.map(|p|p.1)].into_iter().flatten() {
            if texture.width()!=size.0 || texture.height()!=size.1 || texture.depth_or_array_layers()!=1
                || texture.sample_count()!=1 || texture.dimension()!=wgpu::TextureDimension::D2
                || texture.format()!=wgpu::TextureFormat::Rgba8Unorm
                || !texture.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING) {
                bail!("Magnifier layers require matching single-layer sampled RGBA8 textures");
            }
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Magnifier content"),
            source: wgpu::ShaderSource::Wgsl(format!("{}\n{}\n{}\n{}\n{}",
                SAMPLING_SHADER, include_str!("blue_noise.wgsl"), include_str!("content_pass.wgsl"), include_str!("rim_lighting.wgsl"), include_str!("pool_lighting.wgsl")).into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Magnifier content"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magnifier content geometry"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Magnified content"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Magnifier content sampling"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Magnifier content"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &input.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &output.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(
                    &input.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(
                    &layers.map_or(input,|p|p.0).create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(
                    &layers.map_or(input,|p|p.1).create_view(&Default::default())) },
            ],
        });
        Ok(Self {
            pipeline,
            binding,
            uniforms,
            output,
            size,
            device: device.clone(), input: input.clone(),
            foreground:layers.map(|p|p.0.clone()),background:layers.map(|p|p.1.clone()),straight_output:false,sampler, filtered: OnceLock::new(),
        })
    }

    /// Export consumers expect straight RGBA; keep conversion on the GPU.
    pub fn with_straight_output(mut self)->Self {self.straight_output=true;self}

    pub fn output(&self) -> &wgpu::Texture {
        &self.output
    }

    /// One submission per frame: queue writes precede the supplied encoder's
    /// dispatch. Reuse the pipeline, bindings and textures across frames.
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        lens: ContentLens,
    ) -> Result<()> {
        self.encode_with_timestamps(queue,encoder,lens,None)
    }

    fn encode_with_timestamps(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        lens: ContentLens,
        timestamps: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) -> Result<()> {
        if ![
            lens.center[0],
            lens.center[1],
            lens.radius,
            lens.zoom,
            lens.presence,
            lens.bevel,
            lens.shadow_alpha,
            lens.shadow_reach,
            lens.light_direction[0],
            lens.light_direction[1],
            lens.light_tilt,
            lens.aberration,
            lens.reflection,
            lens.source_pixel_scale,
            lens.lighting,
            lens.shine_exponent,
        ]
        .iter()
        .all(|value| value.is_finite())
            || lens.radius < 0.0
            || lens.bevel < 0.0
            || !(0.0..=1.0).contains(&lens.shadow_alpha)
            || lens.shadow_reach < 0.0
            || lens.light_tilt < 0.0
            || !(0.0..=1.0).contains(&lens.aberration)
            || lens.reflection < 0.0
            || !(0.0..=1.0).contains(&lens.lighting)
            || lens.shine_exponent <= 0.0
            || lens.source_pixel_scale <= 0.0
            || lens.zoom <= 0.0
            || !(0.0..=1.0).contains(&lens.presence)
        {
            bail!("Invalid magnifier content geometry");
        }
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                center: lens.center,
                radius: lens.radius,
                zoom: lens.zoom,
                presence: lens.presence,
                bevel: lens.bevel,
                shadow_alpha: lens.shadow_alpha,
                shadow_reach: lens.shadow_reach,
                light_direction: lens.light_direction,
                light_tilt: lens.light_tilt,
                aberration: lens.aberration,
                reflection: lens.reflection,
                source_pixel_scale: lens.source_pixel_scale,
                lighting: lens.lighting,
                shine_exponent: lens.shine_exponent,
                separate_layers: if self.foreground.is_some() {if self.straight_output {2.0} else {1.0}} else {0.0},padding:[0.0;3],
            }),
        );
        // Flat lenses incur no pyramid allocation or per-frame downsampling.
        let binding = if lens.presence > 0.0 && lens.radius > 0.0 && lens.bevel > 0.0 {
            let filtered = self.filtered.get_or_init(|| {
                let pyramid = MipPyramid::new(&self.device, self.foreground.as_ref().unwrap_or(&self.input)).map_err(|e| e.to_string())?;
                let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Filtered magnifier content"), layout: &self.pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&self.input.create_view(&Default::default()))},
                        wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::Sampler(&self.sampler)},
                        wgpu::BindGroupEntry {binding:2,resource:wgpu::BindingResource::TextureView(&self.output.create_view(&Default::default()))},
                        wgpu::BindGroupEntry {binding:3,resource:self.uniforms.as_entire_binding()},
                        wgpu::BindGroupEntry {binding:4,resource:wgpu::BindingResource::TextureView(&pyramid.texture().create_view(&Default::default()))},
                        wgpu::BindGroupEntry {binding:5,resource:wgpu::BindingResource::TextureView(&self.foreground.as_ref().unwrap_or(&self.input).create_view(&Default::default()))},
                        wgpu::BindGroupEntry {binding:6,resource:wgpu::BindingResource::TextureView(&self.background.as_ref().unwrap_or(&self.input).create_view(&Default::default()))},
                    ],
                });
                Ok((pyramid, binding))
            }).as_ref().map_err(|message| anyhow::anyhow!("{message}"))?;
            filtered.0.encode(encoder);
            &filtered.1
        } else { &self.binding };
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Magnifier content"),
            timestamp_writes: timestamps,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, binding, &[]);
        pass.dispatch_workgroups(self.size.0.div_ceil(8), self.size.1.div_ceil(8), 1);
        Ok(())
    }
}

#[cfg(test)]
#[path = "content_pass_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "content_pass_benchmark.rs"]
mod benchmark;

#[cfg(test)]
#[path = "preview_parity_tests.rs"]
mod preview_parity_tests;

#[cfg(test)]
#[path = "layer_sampling_tests.rs"]
mod layer_sampling_tests;
