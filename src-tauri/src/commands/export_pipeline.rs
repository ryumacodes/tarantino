//! Export pipeline commands

use crate::export::{
    self, ExportConfig, ExportFormat, ExportPhase, ExportProgress,
    Project, ProjectEffects, ProjectTracks, QualityPreset, VideoCodec, VideoSettings,
};

#[tauri::command]
pub async fn export_start_pipeline(
    project_path: String,
    output_path: String,
    quality_preset: String,
    format: String,
) -> Result<String, String> {
    println!("Starting export pipeline: {} -> {}", project_path, output_path);

    let preset = match quality_preset.as_str() {
        "draft" => QualityPreset::Draft,
        "standard" => QualityPreset::Standard,
        "high" => QualityPreset::High,
        "production" => QualityPreset::Production,
        _ => QualityPreset::Standard,
    };

    let format_enum = match format.as_str() {
        "mp4" => ExportFormat::MP4,
        "webm" => ExportFormat::WebM,
        "mov" => ExportFormat::MOV,
        "gif" => ExportFormat::GIF,
        _ => ExportFormat::MP4,
    };

    // TODO: Load project from project_path
    let project = Project {
        id: "placeholder".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        clips: vec![],
        tracks: ProjectTracks {
            display: None,
            camera: None,
            audio: None,
        },
        cursor_events: vec![],
        effects: ProjectEffects {
            zoom_segments: vec![],
        },
    };

    let config = export::create_export_config(output_path, preset, format_enum);

    match export::create_export_pipeline(project, config) {
        Ok(_pipeline) => {
            let export_id = format!("export_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis());

            println!("Export pipeline started with ID: {}", export_id);
            Ok(export_id)
        }
        Err(e) => Err(format!("Failed to start export: {}", e))
    }
}

#[tauri::command]
pub async fn export_get_progress(export_id: String) -> Result<ExportProgress, String> {
    println!("Getting export progress for: {}", export_id);

    // TODO: Get actual progress from running export
    Ok(ExportProgress {
        phase: ExportPhase::ProcessingVideo,
        current_frame: 150,
        total_frames: 900,
        elapsed_seconds: 30.0,
        estimated_remaining_seconds: 150.0,
        export_speed_fps: 5.0,
    })
}

#[tauri::command]
pub async fn export_cancel(export_id: String) -> Result<(), String> {
    println!("Cancelling export: {}", export_id);
    // TODO: Cancel running export pipeline
    Ok(())
}

#[tauri::command]
pub async fn export_create_config(
    output_path: String,
    width: u32,
    height: u32,
    fps: f64,
    bitrate_kbps: u32,
    quality_preset: String,
) -> Result<ExportConfig, String> {
    println!("Creating export config: {}x{} @ {} fps", width, height, fps);

    let preset = match quality_preset.as_str() {
        "draft" => QualityPreset::Draft,
        "standard" => QualityPreset::Standard,
        "high" => QualityPreset::High,
        "production" => QualityPreset::Production,
        _ => QualityPreset::Standard,
    };

    let mut config = ExportConfig {
        output_path,
        video: VideoSettings {
            width,
            height,
            fps,
            bitrate_kbps,
            codec: VideoCodec::H264,
            pixel_format: "yuv420p".to_string(),
        },
        quality_preset: preset.clone(),
        ..Default::default()
    };

    preset.apply_to_config(&mut config);

    Ok(config)
}
