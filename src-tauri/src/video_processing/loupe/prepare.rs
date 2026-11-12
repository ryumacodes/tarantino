//! Adapt persisted zoom blocks and original source pointer samples to lens motion.
use super::{animation, timeline::LoupeRange, LoupeTarget, SourceCrop};
use crate::{
    cursor_renderer::{CursorEvent, SpringConfig},
    video_processing::{
        source_crop::remap_motion,
        types::{ZoomBlock, ExportSettings},
        zoom_trajectory::{resolve_center_at_time, targets, CameraViewport},
    },
};
use anyhow::Result;

/// Preview and export must group activity using the same cropped source plane.
pub fn trajectory_with_settings(
    blocks: &[ZoomBlock], events: &[CursorEvent], source: (u32, u32),
    duration_ms: u64, fps: f64, spring: SpringConfig, settings: &ExportSettings,
    prepared_activity: Option<&crate::video_processing::camera_activity::Tracks>,
) -> Result<Vec<LoupeTarget>> {
    let viewport = CameraViewport::from_source_settings(settings, source)?;
    trajectory_in_viewport(blocks, events, source, settings.source_crop, duration_ms, fps,
        spring, viewport,Some(targets::CameraActivity {
            events,prepared:prepared_activity,smooth:crate::video_processing::export::get_cursor_spring_config(settings).mass>0.0 }))
}

pub fn trajectory(
    blocks: &[ZoomBlock],
    events: &[CursorEvent],
    source: (u32, u32),
    crop: Option<SourceCrop>,
    duration_ms: u64,
    fps: f64,
    spring: SpringConfig,
) -> Result<Vec<LoupeTarget>> {
    trajectory_in_viewport(blocks, events, source, crop, duration_ms, fps, spring, CameraViewport::default(),None)
}

/// Resolve pointer grouping and crop mapping once for both overlap targets and animation.
pub fn target_and_animated_trajectories(
    blocks: &[ZoomBlock], events: &[CursorEvent], source: (u32, u32),
    duration_ms: u64, spring: SpringConfig, settings: &ExportSettings,
    prepared_activity: Option<&crate::video_processing::camera_activity::Tracks>,
) -> Result<(Vec<LoupeTarget>, Vec<LoupeTarget>)> {
    let viewport = CameraViewport::from_source_settings(settings, source)?;
    let mut frames = trajectories_in_viewport(blocks, events, source, settings.source_crop,
        duration_ms, animation::SIMULATION_FPS,
        &[SpringConfig { mass: 0.0, ..spring }, spring],
        viewport,Some(targets::CameraActivity {
            events,prepared:prepared_activity,smooth:crate::video_processing::export::get_cursor_spring_config(settings).mass>0.0 }))?;
    let animated = frames.pop().unwrap();
    let targets = frames.pop().unwrap();
    Ok((targets, animated))
}

fn trajectory_in_viewport(
    blocks: &[ZoomBlock], events: &[CursorEvent], source: (u32, u32),
    crop: Option<SourceCrop>, duration_ms: u64, fps: f64, spring: SpringConfig,
    viewport: CameraViewport,
    activity: Option<targets::CameraActivity<'_>>,
) -> Result<Vec<LoupeTarget>> {
    Ok(trajectories_in_viewport(blocks, events, source, crop, duration_ms, fps,
        &[spring], viewport,activity)?.pop().unwrap())
}

fn trajectories_in_viewport(
    blocks: &[ZoomBlock], events: &[CursorEvent], source: (u32, u32),
    crop: Option<SourceCrop>, duration_ms: u64, fps: f64, springs: &[SpringConfig],
    viewport: CameraViewport,
    activity: Option<targets::CameraActivity<'_>>,
) -> Result<Vec<Vec<LoupeTarget>>> {
    let crop = crop.unwrap_or(SourceCrop {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    });
    let resolved = crop.geometry(source.0, source.1)?;
    let mut blocks = blocks.to_vec();
    let mut events = events.to_vec();
    events.sort_by_key(|event| event.timestamp_ms);
    remap_motion(Some(resolved), &mut events, &mut blocks);
    let mut raw_events=activity.map(|input|input.events.to_vec());
    if let Some(raw)=&mut raw_events {
        raw.sort_by_key(|event|event.timestamp_ms);
        remap_motion(Some(resolved),raw,&mut []);
    }
    let activity=activity.map(|input|targets::CameraActivity {
        events:raw_events.as_deref().unwrap(),smooth:input.smooth,prepared:input.prepared });
    for block in &mut blocks {
        block.centers.sort_by(|a,b| a.time.total_cmp(&b.time));
    }
    let ranges: Vec<_> = blocks
        .iter()
        .map(|block| LoupeRange {
            start_ms: block.start_time_ms,
            end_ms: block.end_time_ms,
            disabled: block.is_disabled,
            instant: block.instant_animation,
            options: block.loupe.map(|style| style.options(block.zoom_level)),
        })
        .collect();
    let prepared: Vec<_> = blocks
        .iter()
        .map(|block| targets::prepare_in_viewport(block, &events, viewport,activity))
        .collect();
    // Editor lead-in is a global setting transported on each block.
    let lead_in = blocks
        .iter()
        .map(|block| block.animation_lead_in_ms)
        .max()
        .unwrap_or(0)
        .min(2000);
    springs.iter().map(|&spring| animation::simulate(
        &ranges,
        source,
        resolved.normalized(),
        duration_ms,
        fps,
        lead_in,
        spring,
        |index, time| {
            let block = &blocks[index];
            if block.is_manual {
                return (block.center_x, block.center_y);
            }
            targets::at(&prepared[index], time).unwrap_or_else(|| {
                resolve_center_at_time(
                    &block.centers,
                    time,
                    block.center_x,
                    block.center_y,
                    block.kind.as_deref() == Some("typing"),
                )
            })
        },
    )).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn cropped_proportional_window_uses_the_visible_plane_for_lens_activity() {
        use super::*;
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":1000,"height":1000},
            "screen_width":1000,"screen_height":1000,"window_width":1000,"window_height":1000,
            "visual_settings":{"window_layout_mode":"desktop"},
            "source_crop":{"x":0.5,"y":0.0,"width":0.5,"height":1.0}
        })).unwrap();
        let blocks: Vec<ZoomBlock> = serde_json::from_value(serde_json::json!([{
            "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,"center_x":0.55,"center_y":0.5,
            "instant_animation":true,"loupe":{"radius":0.25,"glass":false,"bevel":0,
                "chromatic_aberration":0,"shadow_alpha":0}
        }])).unwrap();
        let events = [
            CursorEvent { timestamp_ms:0,x:0.55,y:0.5,is_click:true,is_release:false },
            CursorEvent { timestamp_ms:100,x:0.75,y:0.5,is_click:true,is_release:false },
        ];
        let spring = SpringConfig { tension:170.0,friction:50.0,mass:0.0 };
        let mut cropped_settings = settings.clone();
        cropped_settings.window_width = Some(500);
        let expected = trajectory_in_viewport(&blocks,&events,(1000,1000),settings.source_crop,
            1000,120.0,spring,CameraViewport::from_settings(&cropped_settings,(500,1000)),
            Some(targets::CameraActivity { events:&events,prepared:None,smooth:true })).unwrap();
        let actual = trajectory_with_settings(&blocks,&events,(1000,1000),1000,120.0,spring,&settings,None).unwrap();
        assert_eq!(actual,expected);
        let (targets,animated) = target_and_animated_trajectories(&blocks,&events,(1000,1000),
            1000,spring,&settings,None).unwrap();
        assert_eq!(targets,expected);
        assert_eq!(animated,expected);
    }
    use super::*;
    #[test]
    #[ignore = "local preparation timing workload"]
    fn compare_shared_preparation_cost() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "resolution":{"width":1920,"height":1080}
        })).unwrap();
        let blocks: Vec<ZoomBlock> = (0..30).map(|i| serde_json::from_value(serde_json::json!({
            "start_time_ms":i*2000,"end_time_ms":i*2000+1500,"zoom_level":2,
            "center_x":0.5,"center_y":0.5,
            "loupe":{"radius":0.25,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}
        })).unwrap()).collect();
        let events: Vec<_> = (0..3600).rev().map(|i| CursorEvent { is_release: false,
            timestamp_ms:i*16, x:0.2+(i%60) as f64/100.0,
            y:0.2+(i%40) as f64/100.0, is_click:i%15==0,
        }).collect();
        let spring=SpringConfig{mass:3.0,tension:470.0,friction:70.0};
        let mut separate_ns=0;
        let mut shared_ns=0;
        for _ in 0..5 {
            let start=std::time::Instant::now();
            let targets=trajectory_with_settings(&blocks,&events,(2560,1440),60_000,120.0,
                SpringConfig{mass:0.0,..spring},&settings,None).unwrap();
            let animated=trajectory_with_settings(&blocks,&events,(2560,1440),60_000,120.0,
                spring,&settings,None).unwrap();
            separate_ns+=start.elapsed().as_nanos();
            let start=std::time::Instant::now();
            let shared=target_and_animated_trajectories(&blocks,&events,(2560,1440),60_000,
                spring,&settings,None).unwrap();
            shared_ns+=start.elapsed().as_nanos();
            assert_eq!(shared,(targets,animated));
        }
        println!("60s source, 3600 events, 30 lenses; 5 runs: separate={separate_ns}ns shared={shared_ns}ns");
    }

    #[test]
    fn lens_vertical_grouping_keeps_source_extent_across_stage_aspects() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":608,"height":1080},
            "source_crop":{"x":0.5,"y":0.0,"width":0.5,"height":1.0}
        })).unwrap();
        let block: ZoomBlock = serde_json::from_value(serde_json::json!({
            "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,"center_x":0.75,"center_y":0.2,
            "instant_animation":true,
            "loupe":{"radius":0.25,"glass":false,"bevel":0.1,"chromatic_aberration":0.01,"shadow_alpha":0.35}
        })).unwrap();
        let events = [
            CursorEvent { is_release: false, timestamp_ms:0, x:0.75, y:0.2, is_click:true },
            CursorEvent { is_release: false, timestamp_ms:100, x:0.75, y:0.6, is_click:true },
        ];
        let spring = SpringConfig { tension:170.0, friction:50.0, mass:0.0 };
        let run = |settings: &ExportSettings, fps| trajectory_with_settings(
            &[block.clone()], &events, (3840,1080), 1000, fps, spring, settings,None).unwrap();
        let portrait = run(&settings, 120.0);
        // Sharing preprocessing must preserve both scheduler targets and spring motion.
        let mut animated_block = block.clone();
        animated_block.instant_animation = false;
        let animated_spring = SpringConfig { mass: 3.0, tension: 470.0, friction: 70.0 };
        let (targets, animated) = target_and_animated_trajectories(
            &[animated_block.clone()], &events, (3840,1080), 1000, animated_spring, &settings,None).unwrap();
        assert_eq!(targets, trajectory_with_settings(&[animated_block.clone()], &events, (3840,1080),
            1000, 120.0, SpringConfig { mass: 0.0, ..animated_spring }, &settings,None).unwrap());
        assert_eq!(animated, trajectory_with_settings(&[animated_block.clone()], &events, (3840,1080),
            1000, 120.0, animated_spring, &settings,None).unwrap());
        let mut landscape_settings = settings.clone();
        landscape_settings.resolution = Some(crate::video_processing::types::Resolution { width:1920, height:1080 });
        let landscape = run(&landscape_settings, 120.0);
        // Aspect changes leave vertical source-space thresholds unchanged.
        for index in [0,12] {
            assert!((portrait[index].center_y-landscape[index].center_y).abs()<1e-10);
        }
        assert!((portrait[12].center_y-portrait[0].center_y)>0.35);
        assert_eq!(portrait[0].center_x, 0.75);
        for fps in [24.0,30.0,60.0] {
            for (index, frame) in run(&settings, fps).iter().enumerate() {
                let sample = (index as f64*120.0/fps+1e-7).floor() as usize;
                assert_eq!(*frame, portrait[sample]);
            }
        }
    }

    #[test]
    fn cropped_manual_lens_and_classic_camera_keep_separate_paths() {
        let block: ZoomBlock = serde_json::from_value(serde_json::json!({
            "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,"center_x":0.5,"center_y":0.5,
            "is_manual":true,"instant_animation":true,
            "loupe":{"radius":0.25,"glass":true,"bevel":0.1,"chromatic_aberration":0.01,"shadow_alpha":0.35}
        })).unwrap();
        let spring = SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        };
        let crop = SourceCrop {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let frames = trajectory(
            &[block.clone()],
            &[],
            (1000, 500),
            Some(crop),
            1000,
            30.0,
            spring,
        )
        .unwrap();
        assert_eq!(frames[0].center_x, 0.75);
        assert_eq!((frames[0].radius, frames[0].zoom), (0.125, 2.0));
        let camera = crate::video_processing::zoom_trajectory::simulate_zoom_trajectory(
            &[block],
            &[],
            &spring,
            false,
            "mellow",
            30.0,
            1000,
            false,
        );
        assert!(camera.iter().all(|frame| frame.scale == 1.0));
    }

    #[test]
    fn automatic_pointer_and_typing_centers_are_mapped_through_crop_once() {
        let spring = SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 0.0,
        };
        let crop = SourceCrop {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        for kind in ["click", "typing"] {
            let block: ZoomBlock = serde_json::from_value(serde_json::json!({
                "start_time_ms":0,"end_time_ms":1000,"zoom_level":2,"center_x":0.75,"center_y":0.5,
                "kind":kind,"centers":[{"x":0.75,"y":0.5,"time":0}],
                "loupe":{"radius":0.25,"glass":false,"bevel":0.1,"chromatic_aberration":0.01,"shadow_alpha":0.35}
            })).unwrap();
            let events = [CursorEvent { is_release: false,
                timestamp_ms: 0,
                x: 0.75,
                y: 0.5,
                is_click: false,
            }];
            let frames = trajectory(
                &[block],
                &events,
                (1000, 500),
                Some(crop),
                1000,
                30.0,
                spring,
            )
            .unwrap();
            assert_eq!(frames[0].center_x, 0.75, "{kind}");
        }
    }
}
