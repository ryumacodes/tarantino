use super::*;

#[test]
#[ignore = "requires native GPU access; verifies actual magnifier content pixels"]
fn magnifier_content_keeps_outside_pixels_and_samples_inside_on_gpu() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Magnifier pixel regression"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .unwrap();
    for (width, height) in [(96, 64), (64, 96), (97, 65)] {
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let input = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Coordinate pattern"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&[(x * 2) as u8, (y * 2) as u8, 80, 255]);
            }
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &input,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent,
        );
        let pass = ContentPass::new(&device, &input, (width, height)).unwrap();
        let lens = ContentLens {
            center: [width as f32 / 2.0, height as f32 / 2.0],
            radius: 20.0,
            zoom: 2.0,
            presence: 1.0,
            bevel: 0.0,
            ..Default::default()
        };
        for state in [
            ContentLens {
                presence: 0.0,
                ..lens
            },
            ContentLens { zoom: 1.0, ..lens },
            lens,
            ContentLens {
                presence: 0.5,
                ..lens
            },
            ContentLens {
                center: [0.0, 0.0],
                ..lens
            },
        ] {
            let rendered = render(&device, &queue, &pass, state, width, height);
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;
                    let base = &pixels[offset..offset + 4];
                    let actual = &rendered[offset..offset + 4];
                    let distance = ((x as f32 + 0.5 - state.center[0]).powi(2)
                        + (y as f32 + 0.5 - state.center[1]).powi(2))
                    .sqrt();
                    if state.presence == 0.0 || state.zoom == 1.0 || distance > state.radius + 0.5 {
                        assert_eq!(actual, base, "outside/identity pixel {x},{y}");
                    } else if distance < state.radius - 0.5 {
                        let sample_x = (state.center[0]
                            + (x as f32 + 0.5 - state.center[0]) / state.zoom
                            - 0.5)
                            .clamp(0.0, width as f32 - 1.0)
                            * 2.0;
                        let sample_y = (state.center[1]
                            + (y as f32 + 0.5 - state.center[1]) / state.zoom
                            - 0.5)
                            .clamp(0.0, height as f32 - 1.0)
                            * 2.0;
                        for (channel, sample) in [(0, sample_x), (1, sample_y)] {
                            let expected = base[channel] as f32 * (1.0 - state.presence)
                                + sample * state.presence;
                            assert!(
                                (actual[channel] as f32 - expected).abs() <= 1.1,
                                "magnified {x},{y} channel {channel}: {} expected {expected}",
                                actual[channel]
                            );
                        }
                        assert_eq!(&actual[2..], &[80, 255]);
                    }
                }
            }
        }
        assert!(pass.filtered.get().is_none(), "flat lenses must not allocate a pyramid");
        // Align the lens center with a texel center for analytic radial probes.
        let curved = ContentLens {
            center: [(width / 2) as f32 + 0.5, (height / 2) as f32 + 0.5],
            bevel: 8.0,
            ..lens
        };
        let flat = render(&device, &queue, &pass, ContentLens { bevel: 0.0, ..curved }, width, height);
        let refracted = render(&device, &queue, &pass, curved, width, height);
        for (radius, inward_bend) in [(0, 0.0), (12, 0.0), (16, 0.7698004), (18, 2.2677868), (19, 4.2172485)] {
            for direction in [-1, 1] {
                let x = (width as i32 / 2 + direction * radius) as usize;
                let y = height as usize / 2;
                let offset = (y * width as usize + x) * 4;
                let expected_x = width as f32 / 2.0 - (width % 2) as f32 / 2.0
                    + direction as f32 * ((radius as f32 - inward_bend) / 2.0);
                assert!((refracted[offset] as f32 - expected_x * 2.0).abs() <= 1.1,
                    "rim probe radius {radius}, side {direction}: {} vs {}", refracted[offset], expected_x * 2.0);
                assert_eq!(&refracted[offset + 1..offset + 4], &flat[offset + 1..offset + 4]);
            }
        }
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 + 0.5 - curved.center[0];
                let dy = y as f32 + 0.5 - curved.center[1];
                if dx.hypot(dy) <= 12.0 || dx.hypot(dy) >= 20.0 {
                    let offset = ((y * width + x) * 4) as usize;
                    assert_eq!(&refracted[offset..offset+4], &flat[offset..offset+4]);
                }
            }
        }
        // The bend is expressed in the magnified layer. At a fixed rim pixel,
        // both the radial position and bend inverse-map through its zoom.
        for zoom in [1.0,4.0] {
            let state=ContentLens {zoom,..curved};
            let image=render(&device,&queue,&pass,state,width,height);
            for direction in [-1,1] {
                let x=(width as i32/2+direction*18) as usize;
                let y=height as usize/2;
                let offset=(y*width as usize+x)*4;
                let sample=width as f32/2.0-(width%2) as f32/2.0
                    +direction as f32*(18.0-2.2677868)/zoom;
                assert!((image[offset] as f32-sample*2.0).abs()<=1.1,
                    "zoom={zoom} inverse-mapped bend: {} expected {}",image[offset],sample*2.0);
            }
        }
        let shadow = ContentLens { zoom: 1.0, bevel: 0.0, shadow_alpha: 0.5,
            shadow_reach: 20.0, light_direction: [0.0,-1.0], light_tilt: 1.0, ..curved };
        let shaded = render(&device, &queue, &pass, shadow, width, height);
        // Light above: at 25px below the center the thrown shadow is full;
        // at 25px above, reach falloff is 1/3 and lit-side retention is 0.6.
        for (dy, alpha) in [(25,0.5_f32),(-25,1.0/30.0),(0,0.0)] {
            let x = width as usize / 2;
            let y = (height as i32 / 2+dy) as usize;
            let offset = (y * width as usize + x)*4;
            for channel in 0..3 {
                let noise = threshold(x+4,y+4)*alpha.mul_add(64.0,0.0).min(1.0);
                let opacity = alpha+(threshold(x,y)-0.5)*(1.5/255.0)*alpha;
                let expected = pixels[offset+channel] as f32*(1.0-opacity)+noise;
                assert!((shaded[offset+channel] as f32-expected).abs() <= 1.1,
                    "directional shadow dy={dy} channel={channel}: {} expected {expected}",shaded[offset+channel]);
            }
            assert_eq!(shaded[offset+3],255);
        }
        for y in 0..height as usize {
            for x in 0..width as usize {
                let dx=x as f32+0.5-shadow.center[0];
                let dy=y as f32+0.5-shadow.center[1];
                let distance=dx.hypot(dy);
                if distance <= 20.5 { continue; }
                let reach=((35.0-dx.hypot(dy-5.0))/15.0).clamp(0.0,1.0);
                let alpha=0.5*(0.8+0.2*dy/distance)*reach*reach;
                let opacity=alpha+(threshold(x,y)-0.5)*1.5/255.0*alpha;
                let noise=threshold(x+4,y+4)*(alpha*64.0).min(1.0);
                let offset=(y*width as usize+x)*4;
                for channel in 0..3 {
                    let expected=pixels[offset+channel] as f32*(1.0-opacity)+noise;
                    assert!((shaded[offset+channel] as f32-expected).abs()<=1.1,
                        "dither {x},{y}/{channel}: {} expected {expected}",shaded[offset+channel]);
                }
            }
        }
        let invisible = render(&device, &queue, &pass, ContentLens {presence:0.0,..shadow},width,height);
        assert_eq!(invisible,pixels,"inactive lens must not leave a shadow");
        let spectral = ContentLens {bevel:16.0,aberration:1.0,..curved};
        let uncolored = render(&device,&queue,&pass,ContentLens {aberration:0.0,..spectral},width,height);
        let fringed = render(&device,&queue,&pass,spectral,width,height);
        // Independent midpoint quadrature of the compact spectral bumps
        // (10,000 bins per stratum) gives this right-rim red-channel value.
        let expected_red = if width == 64 {72.02357} else {104.01430};
        let probe=((height/2*width+width/2+19)*4) as usize;
        assert!((fringed[probe] as f32-expected_red).abs()<=1.1,
            "spectral band weights/squared-color blend: {} expected {expected_red}",fringed[probe]);
        let mut separated = 0;
        for y in 0..height {
            for x in 0..width {
                let offset=((y*width+x)*4) as usize;
                let distance=(x as f32+0.5-spectral.center[0]).hypot(y as f32+0.5-spectral.center[1]);
                if distance <= 4.0 || distance >= 20.0 {
                    assert_eq!(&fringed[offset..offset+4],&uncolored[offset..offset+4]);
                }
                assert_eq!(fringed[offset+2],80,"constant blue must survive spectral integration");
                assert_eq!(fringed[offset+3],255);
                if fringed[offset].abs_diff(uncolored[offset])>1 || fringed[offset+1].abs_diff(uncolored[offset+1])>1 {
                    separated += 1;
                }
            }
        }
        assert!(separated>10,"spectral rim did not separate gradient channels");
        let reflective=ContentLens {reflection:1.0,..curved};
        let reflected=render(&device,&queue,&pass,reflective,width,height);
        for direction in [-1,1] {
            let x=(width as i32/2+direction*19) as usize;
            let y=height as usize/2;
            let offset=(y*width as usize+x)*4;
            // Fresnel weight .09800945, reflected radial read 25.1466567.
            let expected=(width/2*2) as f32+direction as f32*18.263122;
            assert!((reflected[offset] as f32-expected).abs()<=1.1,
                "plain-scene rim reflection: {} expected {expected}",reflected[offset]);
            assert_eq!(&reflected[offset+1..offset+4],&refracted[offset+1..offset+4]);
        }
        for y in 0..height {
            for x in 0..width {
                let distance=(x as f32+0.5-curved.center[0]).hypot(y as f32+0.5-curved.center[1]);
                if distance<=8.0 || distance>=20.0 {
                    let offset=((y*width+x)*4) as usize;
                    assert_eq!(&reflected[offset..offset+4],&refracted[offset..offset+4]);
                }
            }
        }
        // Optical minimum widths/reach are source pixels, even when a source
        // pixel occupies many stage pixels. These vectors include a clamped
        // reflection minimum radius and subpixel source bevel strength.
        for (scale,delta) in [(0.5,18.263122),(10.0,15.846245),(20.0,13.663907)] {
            if width==64 && scale==20.0 { continue; } // reflected read would hit the texture edge
            let scaled=ContentLens {source_pixel_scale:scale,..reflective};
            let image=render(&device,&queue,&pass,scaled,width,height);
            let offset=((height/2*width+width/2+19)*4) as usize;
            let expected=(width/2*2) as f32+delta;
            assert!((image[offset] as f32-expected).abs()<=1.1,
                "source pixel scale {scale}: {} expected {expected}",image[offset]);
        }
        assert!(pass.filtered.get().is_some(), "curved lenses must reuse a filtered pyramid");
        // Fine alternating detail should lose alias contrast only where the
        // curved surface compresses several input pixels into one output pixel.
        let checker:Vec<u8>=(0..height).flat_map(|y|(0..width).flat_map(move |x|
            [if (x+y)%2==0 {0} else {255},80,80,255])).collect();
        queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,
            origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&checker,
            wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},extent);
        let state=ContentLens {zoom:1.0,..curved};
        let filtered=render(&device,&queue,&pass,state,width,height);
        let mut filtered_contrast=0.0f64;
        let mut unfiltered_contrast=0.0f64;
        let mut probes=0;
        for y in 0..height {for x in 0..width {
            let dx=f64::from(x)+0.5-f64::from(state.center[0]);
            let dy=f64::from(y)+0.5-f64::from(state.center[1]);
            let distance=dx.hypot(dy);
            let offset=((y*width+x)*4) as usize;
            if distance>=20.5 || distance<=12.0 {
                assert_eq!(&filtered[offset..offset+4],&checker[offset..offset+4]);
            }
            if !(19.0..=19.5).contains(&distance) {continue;}
            let t=(distance-12.0)/8.0;
            let bend=(t*t/(1.0-(t*t).min(0.99)).sqrt()).min(6.0)*8.0/3.0;
            let sx=f64::from(x)-dx/distance*bend;
            let sy=f64::from(y)-dy/distance*bend;
            let mut value=0.0;
            for iy in 0..2 {for ix in 0..2 {
                let px=(sx.floor() as i64+ix).clamp(0,i64::from(width)-1);
                let py=(sy.floor() as i64+iy).clamp(0,i64::from(height)-1);
                let wx=if ix==0 {1.0-sx.fract()} else {sx.fract()};
                let wy=if iy==0 {1.0-sy.fract()} else {sy.fract()};
                value+=if (px+py)%2==0 {0.0} else {255.0*wx*wy};
            }}
            unfiltered_contrast+=(value-127.5).abs();
            filtered_contrast+=(f64::from(filtered[offset])-127.5).abs();
            probes+=1;
        }}
        assert!(probes>=20);
        assert!(filtered_contrast<unfiltered_contrast*0.8,
            "rim alias contrast {filtered_contrast} must improve over {unfiltered_contrast}");
        for color in [[128u8,96,64,255],[40,60,80,128]] {
            let constant=color.repeat((width*height) as usize);
            queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,
                origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&constant,
                wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},extent);
            let neutral=render(&device,&queue,&pass,spectral,width,height);
            for (actual,expected) in neutral.iter().zip(&constant) {
                assert!(actual.abs_diff(*expected)<=1,"spectral integration changed constant premultiplied color");
            }
        }
        for color in [[0u8,0,0,0],[40,60,80,128],[255,255,255,255]] {
            let constant=color.repeat((width*height) as usize);
            queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,
                origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&constant,
                wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},extent);
            for exponent in [1.5,8.0,24.0] {for presence in [0.5,1.0] {
                let state=ContentLens {lighting:1.0,shine_exponent:exponent,presence,shadow_alpha:0.35,shadow_reach:40.0,light_tilt:1.0,..curved};
                let actual=render(&device,&queue,&pass,state,width,height);
                for y in 0..height {for x in 0..width {
                    let expected=rim_light_oracle(color,x,y,state);
                    let offset=((y*width+x)*4) as usize;
                    for c in 0..4 {
                        assert!(actual[offset+c].abs_diff(expected[c])<=1,
                            "rim light {width}x{height} ({x},{y}) exponent{exponent} presence{presence} channel{c}: {} vs {}",
                            actual[offset+c],expected[c]);
                    }
                }}
            }}
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        assert!(pass
            .encode(&queue, &mut encoder, ContentLens { zoom: 0.0, ..lens })
            .is_err());
        assert!(pass.encode(&queue,&mut encoder,ContentLens {source_pixel_scale:0.0,..lens}).is_err());
        assert!(ContentPass::new(&device, &input, (width + 1, height)).is_err());
    }
}

pub(super) fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pass: &ContentPass,
    lens: ContentLens,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    pass.encode(queue, &mut encoder, lens).unwrap();
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: pass.output(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap()
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let data = slice.get_mapped_range().unwrap();
    let pixels = data
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..(width * 4) as usize].iter().copied())
        .collect();
    drop(data);
    buffer.unmap();
    pixels
}

// Fixed ordered matrix gives an independent oracle for the shader's bit expansion.
fn threshold(x: usize, y: usize) -> f32 {
    const MATRIX: [[u8;8];8] = [
        [0,32,8,40,2,34,10,42], [48,16,56,24,50,18,58,26],
        [12,44,4,36,14,46,6,38], [60,28,52,20,62,30,54,22],
        [3,35,11,43,1,33,9,41], [51,19,59,27,49,17,57,25],
        [15,47,7,39,13,45,5,37], [63,31,55,23,61,29,53,21],
    ];
    MATRIX[y%8][x%8] as f32/64.0
}

// f64 oracle for the recovered polished-rim model. Constant source colors
// isolate lighting, opacity and highlight rolloff from texture filtering.
fn rim_light_oracle(color:[u8;4],x:u32,y:u32,lens:ContentLens)->[u8;4] {
    let dx=x as f64+0.5-lens.center[0] as f64;
    let dy=y as f64+0.5-lens.center[1] as f64;
    let distance=dx.hypot(dy);
    let radius=lens.radius as f64;
    let smooth=|a:f64,b:f64,v:f64| {let t=((v-a)/(b-a)).clamp(0.0,1.0);t*t*(3.0-2.0*t)};
    let coverage=(1.0-smooth(radius-0.5,radius+0.5,distance))*lens.presence as f64;
    let color=pool_light_oracle(color,dx,dy,coverage,lens);
    if coverage<=0.0 || distance>=radius || distance<0.001 {return color.map(|v|(v*255.0).round() as u8);}
    let band=(radius*0.036*3f64.powf(-0.8)).max(1.0);
    let core=(1.0-(radius-distance)/band).clamp(0.0,1.0).powf(2.5);
    let inner_u=(1.0-(radius-distance)/(band*50.0)).clamp(0.0,1.0);
    let inner=inner_u*inner_u*(3.0-2.0*inner_u)*0.1;
    let facing=(dx*lens.light_direction[0] as f64+dy*lens.light_direction[1] as f64)/distance;
    let softness=(4.0/lens.shine_exponent as f64).clamp(0.0,1.0);
    let arcs=|width:f64,edge:f64| {
        let start=1.0-width;
        let feather=((width*(1.0-edge)+0.02*edge)*(1.0+softness*0.5)).max(0.01);
        [smooth(start,start+feather,facing.max(0.0)),
            0.28*smooth(start+width*0.25,start+feather+width*0.25,(-facing).max(0.0))]
    };
    let edge=arcs(0.3,0.25);let broad=arcs(0.55,0.125);
    let primary=edge[0]*core+broad[0]*inner;
    let secondary=edge[1]*core+broad[1]*inner;
    let weight=coverage*lens.presence as f64*1.05*(1.0+(threshold(x as usize,y as usize) as f64-0.5)*0.8);
    let tint=[1.0,0.98,0.95];let back=[0.9,0.96,1.05];
    let light:[f64;3]=std::array::from_fn(|c|tint[c]*(primary+back[c]*secondary+0.05*(core+inner))*weight);
    let t=((distance-radius+8.0)/8.0).clamp(0.0,1.0);
    let rgb:[f64;3]=std::array::from_fn(|c|color[c]*(1.0-0.14*t*t*coverage)*(1.0+light[c]*0.35)+light[c]*0.55);
    let rolloff=rgb.iter().copied().fold(1.0,f64::max);
    let alpha=(color[3]+light.iter().copied().fold(0.0,f64::max)*0.55).min(1.0);
    [((rgb[0]/rolloff)*255.0).round() as u8,((rgb[1]/rolloff)*255.0).round() as u8,
        ((rgb[2]/rolloff)*255.0).round() as u8,(alpha*255.0).round() as u8]
}

fn pool_light_oracle(color:[u8;4],dx:f64,dy:f64,coverage:f64,lens:ContentLens)->[f64;4] {
    let radius=lens.radius as f64;
    let direction=lens.light_direction.map(f64::from);
    let source_scale=lens.source_pixel_scale as f64;
    let presence=(lens.bevel as f64/source_scale*0.5).min(1.0)*lens.presence as f64;
    let spectrum=|t:f64| {
        let t=t.clamp(0.0,1.0);
        let ramp=(t/0.4).clamp(0.0,1.0);let ramp=ramp*ramp*(3.0-2.0*ramp);
        let angle=(1.0-t.powf(0.6))*std::f64::consts::TAU;
        let warmth=[0.878,-0.023,-0.851];
        let offsets=[0.0,2.0943951,4.1887902];
        std::array::from_fn::<_,3,_>(|c|(1.0+0.175*ramp*(angle-offsets[c]).cos())*(1.0+0.125*warmth[c])*(0.5+0.5*ramp))
    };
    let inner_u=((dx-direction[0]*radius).hypot(dy-direction[1]*radius)/radius-1.0).clamp(0.0,1.0);
    let reach=(lens.shadow_reach as f64).max(source_scale);
    let shift=(radius*0.5).min(reach*0.45);
    let outer_u=((radius-(dx+direction[0]*shift).hypot(dy+direction[1]*shift))/shift).clamp(0.0,1.0);
    let inner=spectrum(inner_u);let outer=spectrum(1.0-outer_u);
    let inner_amount=if coverage>0.001 {inner_u*inner_u*0.25*presence*coverage} else {0.0};
    let outer_amount=outer_u*outer_u*0.3*presence*(1.0-coverage);
    let throw=(lens.light_tilt as f64*reach.min(radius*0.25)).min(reach*0.45);
    let effective=(reach-throw).max(reach*0.4);
    let falloff=(1.0-((dx+direction[0]*throw).hypot(dy+direction[1]*throw)-radius)/effective).clamp(0.0,1.0);
    let facing=(dx*direction[0]+dy*direction[1])/dx.hypot(dy).max(0.001*source_scale);
    let shadow=lens.shadow_alpha as f64*(1.0-0.4*(facing+1.0)*0.5)*falloff*falloff*(1.0-coverage);
    let tint=[1.0,0.98,0.95];
    let light:[f64;3]=std::array::from_fn(|c|(inner[c]*inner_amount+outer[c]*outer_amount)*tint[c]);
    let rgb:[f64;3]=std::array::from_fn(|c|color[c] as f64/255.0*(1.0-shadow+light[c]*0.65)+light[c]*0.35);
    let rolloff=rgb.iter().copied().fold(1.0,f64::max);
    let alpha=color[3] as f64/255.0;
    [rgb[0]/rolloff,rgb[1]/rolloff,rgb[2]/rolloff,
        (alpha+(1.0-alpha)*shadow+light.iter().copied().fold(0.0,f64::max)*0.35).min(1.0)]
}
