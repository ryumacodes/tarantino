//! Texture management for the compositor
//!
//! Handles creating and updating GPU textures for display, camera, and cursor.

use anyhow::Result;
use wgpu::*;

use super::{CursorData, VideoFormat, VideoFrameData};

/// Update or create a display/camera texture with new frame data
pub fn update_frame_texture(
    device: &Device,
    queue: &Queue,
    existing_texture: &mut Option<Texture>,
    frame: &VideoFrameData,
    label: &str,
) -> Result<()> {
    match frame.format {
        VideoFormat::Rgba8 => {
            // Create or update texture if dimensions changed
            if existing_texture.is_none()
                || existing_texture.as_ref().unwrap().width() != frame.width
                || existing_texture.as_ref().unwrap().height() != frame.height
            {
                *existing_texture = Some(device.create_texture(&TextureDescriptor {
                    label: Some(label),
                    size: Extent3d {
                        width: frame.width,
                        height: frame.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8UnormSrgb,
                    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                    view_formats: &[],
                }));
            }

            // Upload frame data
            if let Some(texture) = existing_texture {
                queue.write_texture(
                    ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    &frame.data,
                    ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(frame.width * 4),
                        rows_per_image: Some(frame.height),
                    },
                    Extent3d {
                        width: frame.width,
                        height: frame.height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        VideoFormat::Yuv420 | VideoFormat::Nv12 => {
            // TODO: Implement YUV to RGB conversion
            return Err(anyhow::anyhow!("YUV formats not yet implemented"));
        }
    }

    Ok(())
}

/// Update cursor texture with new cursor data
pub fn update_cursor_texture(
    device: &Device,
    queue: &Queue,
    existing_texture: &mut Option<Texture>,
    cursor: &CursorData,
) -> Result<()> {
    if !cursor.visible || cursor.image.is_empty() {
        return Ok(());
    }

    // Create or update cursor texture if dimensions changed
    if existing_texture.is_none()
        || existing_texture.as_ref().unwrap().width() != cursor.width
        || existing_texture.as_ref().unwrap().height() != cursor.height
    {
        *existing_texture = Some(device.create_texture(&TextureDescriptor {
            label: Some("Cursor Texture"),
            size: Extent3d {
                width: cursor.width,
                height: cursor.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }));
    }

    // Upload cursor image
    if let Some(texture) = existing_texture {
        queue.write_texture(
            ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &cursor.image,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(cursor.width * 4),
                rows_per_image: Some(cursor.height),
            },
            Extent3d {
                width: cursor.width,
                height: cursor.height,
                depth_or_array_layers: 1,
            },
        );
    }

    Ok(())
}

/// Create a placeholder 1x1 transparent texture
pub fn create_placeholder_texture(device: &Device, queue: &Queue) -> Texture {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Placeholder Texture"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload transparent black pixel
    queue.write_texture(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &[0, 0, 0, 0], // Transparent black
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    texture
}

/// Create the output render target texture
pub fn create_output_texture(device: &Device, width: u32, height: u32) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("Compositor Output"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Read output texture data back to CPU
pub async fn read_output_texture(
    device: &Device,
    queue: &Queue,
    output_texture: &Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    // Create a buffer to copy the texture into
    let buffer_size = (width * height * 4) as u64;
    let output_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Output Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy texture to buffer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Texture Copy"),
    });

    encoder.copy_texture_to_buffer(
        ImageCopyTexture {
            texture: output_texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        ImageCopyBuffer {
            buffer: &output_buffer,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Map buffer and read data
    let buffer_slice = output_buffer.slice(..);
    let (sender, receiver) = futures::channel::oneshot::channel();

    buffer_slice.map_async(MapMode::Read, move |result| {
        sender.send(result).ok();
    });

    device.poll(Maintain::Wait);
    receiver.await??;

    let data = buffer_slice.get_mapped_range();
    let result = data.to_vec();
    drop(data);
    output_buffer.unmap();

    Ok(result)
}
