#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use crate::auto_zoom::{ZoomAnalysis, ZoomBlock};
use crate::mouse_tracking::{MouseEvent, MouseEventType};

/// Post-processing configuration for Screen Studio-style effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingConfig {
    pub enable_zoom_effects: bool,
    pub enable_cursor_overlay: bool,
    pub cursor_size: f32,
    pub cursor_style: CursorStyle,
    pub output_quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CursorStyle {
    MacOS,
    Touch,
    Custom { path: String },
}

impl Default for PostProcessingConfig {
    fn default() -> Self {
        Self {
            enable_zoom_effects: true,
            // Cursor overlay is experimental; disable by default until movement is complete
            enable_cursor_overlay: false,
            cursor_size: 1.2,
            cursor_style: CursorStyle::MacOS,
            output_quality: 0.8,
        }
    }
}

/// Post-processing pipeline for applying Screen Studio effects
pub struct PostProcessor {
    config: PostProcessingConfig,
}

impl PostProcessor {
    pub fn new(config: PostProcessingConfig) -> Self {
        Self { config }
    }

    /// Apply post-processing effects to recorded video
    pub async fn process_video(
        &self,
        input_path: &Path,
        output_path: &Path,
        zoom_analysis: Option<&ZoomAnalysis>,
    ) -> Result<()> {
        println!("Starting post-processing pipeline (Screen Studio style)");
        println!("Input: {}", input_path.display());
        println!("Output: {}", output_path.display());

        if !input_path.exists() {
            return Err(anyhow::anyhow!("Input video file does not exist"));
        }

        // Step 1: Apply zoom effects if available
        let zoom_processed_path = if self.config.enable_zoom_effects && zoom_analysis.is_some() {
            let temp_path = input_path.with_extension("zoom_temp.mp4");
            self.apply_zoom_effects(input_path, &temp_path, zoom_analysis.unwrap()).await?;
            temp_path
        } else {
            input_path.to_path_buf()
        };

        // Step 2: Apply cursor overlay if enabled
        let final_output = if self.config.enable_cursor_overlay {
            self.apply_cursor_overlay(&zoom_processed_path, output_path).await?;
            output_path.to_path_buf()
        } else {
            // Just copy/move the zoom-processed file to final output
            std::fs::copy(&zoom_processed_path, output_path)?;
            output_path.to_path_buf()
        };

        // Clean up temporary files
        if zoom_processed_path != input_path.to_path_buf() && zoom_processed_path.exists() {
            let _ = std::fs::remove_file(&zoom_processed_path);
        }

        println!("Post-processing completed: {}", final_output.display());
        Ok(())
    }

    /// Get video dimensions using FFprobe
    fn get_video_dimensions(&self, input_path: &Path) -> Result<(u32, u32)> {
        let output = Command::new("ffprobe")
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height",
                "-of", "csv=s=x:p=0",
            ])
            .arg(input_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("FFprobe failed to get video dimensions"));
        }

        let dimensions = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = dimensions.trim().split('x').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid dimensions format from FFprobe"));
        }

        let width: u32 = parts[0].parse().map_err(|_| anyhow::anyhow!("Invalid width"))?;
        let height: u32 = parts[1].parse().map_err(|_| anyhow::anyhow!("Invalid height"))?;

        Ok((width, height))
    }

    /// Apply zoom effects using FFmpeg filters (Screen Studio approach)
    async fn apply_zoom_effects(
        &self,
        input_path: &Path,
        output_path: &Path,
        zoom_analysis: &ZoomAnalysis,
    ) -> Result<()> {
        println!("Applying {} zoom blocks", zoom_analysis.zoom_blocks.len());

        if zoom_analysis.zoom_blocks.is_empty() {
            // No zoom blocks, just copy the file
            std::fs::copy(input_path, output_path)?;
            return Ok(());
        }

        // Get video dimensions for zoompan filter (requires constant values)
        let (width, height) = self.get_video_dimensions(input_path)?;
        println!("Video dimensions: {}x{}", width, height);

        // Build complex FFmpeg filter for zoom effects
        let filter_complex = self.build_zoom_filter(&zoom_analysis.zoom_blocks, zoom_analysis.session_duration, width, height)?;
        
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i").arg(input_path)
            .arg("-filter_complex").arg(&filter_complex)
            .arg("-map").arg("[output]")
            .arg("-c:v").arg("libx264")
            .arg("-preset").arg("medium")
            .arg("-crf").arg("18") // High quality for zoom effects
            .arg("-y") // Overwrite output
            .arg(output_path);

        println!("Zoom effects FFmpeg command: {:?}", cmd);

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("FFmpeg zoom processing failed: {}", stderr));
        }

        println!("Zoom effects applied successfully");
        Ok(())
    }

    /// Build FFmpeg filter chain for zoom blocks (Screen Studio style)
    fn build_zoom_filter(&self, zoom_blocks: &[ZoomBlock], duration_ms: u64, width: u32, height: u32) -> Result<String> {
        if zoom_blocks.is_empty() {
            return Ok("[0:v]copy[output]".to_string());
        }

        let _duration_seconds = duration_ms as f64 / 1000.0;

        // Build a single zoompan filter that handles all zoom blocks
        // FFmpeg zoompan evaluates the expression for each frame, so we can use nested conditionals
        let mut zoom_expr_parts: Vec<String> = Vec::new();
        let mut x_expr_parts: Vec<String> = Vec::new();
        let mut y_expr_parts: Vec<String> = Vec::new();

        for block in zoom_blocks.iter() {
            let start_time = block.start_time as f64 / 1000.0;
            let end_time = block.end_time as f64 / 1000.0;
            let duration = end_time - start_time;

            // Smooth ease-in-out zoom using sine curve
            // zoom = 1 + (zoom_factor - 1) * sin(progress * PI) for smooth bell curve
            let zoom_part = format!(
                "if(between(t,{:.3},{:.3}),1+({:.2}-1)*sin((t-{:.3})/{:.3}*PI),",
                start_time, end_time,
                block.zoom_factor,
                start_time, duration
            );
            zoom_expr_parts.push(zoom_part);

            // Center coordinates (0-1 range converted to pixel positions)
            let x_part = format!(
                "if(between(t,{:.3},{:.3}),{:.4}*iw,",
                start_time, end_time,
                block.center_x - 0.5 / block.zoom_factor as f64
            );
            x_expr_parts.push(x_part);

            let y_part = format!(
                "if(between(t,{:.3},{:.3}),{:.4}*ih,",
                start_time, end_time,
                block.center_y - 0.5 / block.zoom_factor as f64
            );
            y_expr_parts.push(y_part);
        }

        // Close all the if statements with default value of 1 (no zoom) and center
        let zoom_expr = format!(
            "{}1{}",
            zoom_expr_parts.join(""),
            ")".repeat(zoom_expr_parts.len())
        );
        let x_expr = format!(
            "{}iw/2-(iw/zoom/2){}",
            x_expr_parts.join(""),
            ")".repeat(x_expr_parts.len())
        );
        let y_expr = format!(
            "{}ih/2-(ih/zoom/2){}",
            y_expr_parts.join(""),
            ")".repeat(y_expr_parts.len())
        );

        // Build complete zoompan filter
        // Use fps=60 for smooth animation, d=1 means 1 frame per input frame
        // zoompan requires constant numeric dimensions, not expressions like iw*2
        let filter = format!(
            "[0:v]zoompan=z='{}':x='{}':y='{}':d=1:s={}x{}:fps=60[output]",
            zoom_expr, x_expr, y_expr, width * 2, height * 2
        );

        Ok(filter)
    }

    /// Apply cursor overlay using Screen Studio approach
    async fn apply_cursor_overlay(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        println!("Applying Screen Studio-style cursor overlay");
        
        // Load mouse tracking data from sidecar
        let sidecar_path = input_path.with_extension("mp4.sidecar.json");
        let mouse_events = if sidecar_path.exists() {
            match self.load_mouse_events(&sidecar_path).await {
                Ok(events) => events,
                Err(e) => {
                    println!("Warning: Failed to load mouse events: {}, proceeding without cursor", e);
                    std::fs::copy(input_path, output_path)?;
                    return Ok(());
                }
            }
        } else {
            println!("No sidecar file found, proceeding without cursor overlay");
            std::fs::copy(input_path, output_path)?;
            return Ok(());
        };

        // Generate cursor overlay video
        self.create_cursor_overlay_video(input_path, output_path, &mouse_events).await?;
        
        println!("Cursor overlay applied successfully");
        Ok(())
    }

    /// Load mouse events from sidecar file
    async fn load_mouse_events(&self, sidecar_path: &Path) -> Result<Vec<MouseEvent>> {
        use std::fs;
        
        let content = fs::read_to_string(sidecar_path)?;
        let sidecar: serde_json::Value = serde_json::from_str(&content)?;
        
        // Extract mouse events from sidecar
        let events = sidecar["mouse_events"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|event| {
                // Parse mouse event from JSON
                serde_json::from_value(event.clone()).ok()
            })
            .collect();
            
        Ok(events)
    }

    /// Create cursor overlay video using FFmpeg
    async fn create_cursor_overlay_video(
        &self,
        input_path: &Path,
        output_path: &Path,
        mouse_events: &[MouseEvent],
    ) -> Result<()> {
        if mouse_events.is_empty() {
            println!("No mouse events to overlay");
            std::fs::copy(input_path, output_path)?;
            return Ok(());
        }

        // Generate cursor SVG frames
        let cursor_svg = self.generate_cursor_svg()?;
        let temp_dir = std::env::temp_dir().join("tarantino_cursor_overlay");
        std::fs::create_dir_all(&temp_dir)?;
        
        // Save cursor SVG
        let cursor_path = temp_dir.join("cursor.svg");
        std::fs::write(&cursor_path, cursor_svg)?;

        // Generate drawtext filter for cursor movement
        let drawtext_filter = self.build_cursor_movement_filter(mouse_events, &cursor_path)?;

        // Apply cursor overlay with FFmpeg
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i").arg(input_path)
            .arg("-filter_complex").arg(&drawtext_filter)
            .arg("-map").arg("[output]")
            .arg("-c:v").arg("libx264")
            .arg("-preset").arg("medium")
            .arg("-crf").arg("18")
            .arg("-y")
            .arg(output_path);

        println!("Cursor overlay FFmpeg command: {:?}", cmd);

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("FFmpeg cursor overlay failed: {}", stderr));
        }

        // Clean up temp files
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    /// Generate SVG cursor based on style
    fn generate_cursor_svg(&self) -> Result<String> {
        let cursor_svg = match &self.config.cursor_style {
            CursorStyle::MacOS => self.generate_macos_cursor_svg(),
            CursorStyle::Touch => self.generate_touch_cursor_svg(),
            CursorStyle::Custom { path } => {
                // Load custom cursor from path
                std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to load custom cursor: {}", e))?
            }
        };

        Ok(cursor_svg)
    }

    /// Generate macOS-style cursor SVG
    fn generate_macos_cursor_svg(&self) -> String {
        let size = (24.0 * self.config.cursor_size) as i32;
        format!(
            r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
                <defs>
                    <filter id="shadow">
                        <feDropShadow dx="1" dy="1" stdDeviation="2" flood-opacity="0.3"/>
                    </filter>
                </defs>
                <path d="M0,0 L0,16 L5,11 L8,11 L12,19 L15,18 L11,10 L13,10 Z" 
                      fill="white" 
                      stroke="black" 
                      stroke-width="1" 
                      filter="url(#shadow)"/>
            </svg>"#,
            size, size
        )
    }

    /// Generate touch-style cursor SVG (circular)
    fn generate_touch_cursor_svg(&self) -> String {
        let radius = (12.0 * self.config.cursor_size) as i32;
        let size = radius * 2;
        format!(
            r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
                <defs>
                    <filter id="shadow">
                        <feDropShadow dx="0" dy="2" stdDeviation="4" flood-opacity="0.2"/>
                    </filter>
                </defs>
                <circle cx="{}" cy="{}" r="{}" 
                        fill="rgba(255,255,255,0.8)" 
                        stroke="rgba(0,0,0,0.3)" 
                        stroke-width="2"
                        filter="url(#shadow)"/>
            </svg>"#,
            size, size, radius, radius, radius - 2
        )
    }

    /// Build FFmpeg filter for cursor movement
    fn build_cursor_movement_filter(&self, mouse_events: &[MouseEvent], _cursor_path: &Path) -> Result<String> {
        // For now, create a simple overlay at the first click position
        // TODO: Implement full movement tracking with _cursor_path
        if let Some(first_click) = mouse_events.iter().find(|e| matches!(e.event_type, MouseEventType::ButtonPress { .. })) {
            let filter = format!(
                "[0:v]overlay={}:{}[output]",
                first_click.x as i32,
                first_click.y as i32
            );
            Ok(filter)
        } else {
            // No clicks, just pass through
            Ok("[0:v]copy[output]".to_string())
        }
    }
}

/// Helper function to create default post-processor
pub fn create_default_processor() -> PostProcessor {
    PostProcessor::new(PostProcessingConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_filter_generation() {
        let processor = create_default_processor();
        
        let zoom_blocks = vec![
            ZoomBlock {
                id: "zoom_1".to_string(),
                click_x: 0.5,
                click_y: 0.3,
                center_x: 0.5,
                center_y: 0.3,
                start_time: 1000,
                end_time: 2000,
                zoom_factor: 2.0,
                is_manual: false,
                centers: vec![],
                kind: "click".to_string(),
                zoom_in_speed: None,
                zoom_out_speed: None,
            }
        ];

        let filter = processor.build_zoom_filter(&zoom_blocks, 5000, 1920, 1080).unwrap();
        assert!(filter.contains("zoompan"));
        assert!(filter.contains("2"));  // zoom factor
    }
}