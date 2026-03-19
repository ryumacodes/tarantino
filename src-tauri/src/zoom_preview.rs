#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::command;
use crate::mouse_tracking::{MouseEvent, MouseEventType, MouseButton};

/// Lightweight preview zoom indicator (before full processing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewZoomIndicator {
    pub id: String,
    pub click_time: u64,        // When the click occurred (ms)
    pub click_x: f64,           // Click position X (0-1 normalized)
    pub click_y: f64,           // Click position Y (0-1 normalized) 
    pub preview_start: u64,     // Preview zoom start time (ms)
    pub preview_end: u64,       // Preview zoom end time (ms)
    pub confidence: f32,        // Confidence this will become a zoom (0-1)
}

/// Configuration for preview generation
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    pub default_zoom_duration: u64,  // Default 800ms like Screen Studio
    pub min_click_spacing: u64,       // Minimum 500ms between previews
    pub confidence_threshold: f32,    // Only show previews above this confidence
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            default_zoom_duration: 800,
            min_click_spacing: 500,
            confidence_threshold: 0.3, // Show most potential zooms
        }
    }
}

/// Result of preview zoom analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewZoomAnalysis {
    pub indicators: Vec<PreviewZoomIndicator>,
    pub total_clicks: usize,
    pub total_indicators: usize,
    pub session_duration: u64,
    pub analysis_time_ms: u64,
}

/// Preview zoom generator - fast analysis of raw mouse events
pub struct ZoomPreviewGenerator {
    config: PreviewConfig,
}

impl ZoomPreviewGenerator {
    pub fn new(config: PreviewConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(PreviewConfig::default())
    }

    /// Generate preview indicators from raw mouse events
    pub fn generate_preview_indicators(&self, mouse_events: &[MouseEvent], session_duration: u64) -> Result<PreviewZoomAnalysis> {
        let start_time = std::time::Instant::now();

        // Extract click events (button presses only)
        let click_events = self.extract_click_events(mouse_events);
        
        // Generate preview indicators with spacing and confidence
        let indicators = self.create_preview_indicators(&click_events, session_duration)?;
        
        let analysis_time = start_time.elapsed().as_millis() as u64;
        
        Ok(PreviewZoomAnalysis {
            total_indicators: indicators.len(),
            total_clicks: click_events.len(),
            indicators,
            session_duration,
            analysis_time_ms: analysis_time,
        })
    }

    /// Extract click events from mouse events (left clicks only for now)
    fn extract_click_events<'a>(&self, mouse_events: &'a [MouseEvent]) -> Vec<&'a MouseEvent> {
        mouse_events.iter()
            .filter(|event| {
                matches!(event.event_type, 
                    MouseEventType::ButtonPress { button: MouseButton::Left }
                )
            })
            .collect()
    }

    /// Create preview indicators with smart spacing and confidence
    fn create_preview_indicators(&self, click_events: &[&MouseEvent], session_duration: u64) -> Result<Vec<PreviewZoomIndicator>> {
        let mut indicators = Vec::new();
        let mut last_indicator_time = 0u64;

        for (i, event) in click_events.iter().enumerate() {
            // Skip clicks too close to the previous indicator
            if event.timestamp.saturating_sub(last_indicator_time) < self.config.min_click_spacing {
                continue;
            }

            // Calculate confidence based on context
            let confidence = self.calculate_confidence(event, click_events, i);
            
            // Only create indicators above threshold
            if confidence < self.config.confidence_threshold {
                continue;
            }

            // Calculate preview timing (centered on click)
            let half_duration = self.config.default_zoom_duration / 2;
            let preview_start = event.timestamp.saturating_sub(half_duration);
            let preview_end = std::cmp::min(event.timestamp + half_duration, session_duration);

            let indicator = PreviewZoomIndicator {
                id: format!("preview_{}", i),
                click_time: event.timestamp,
                click_x: event.x, // Already normalized in mouse tracking
                click_y: event.y, // Already normalized in mouse tracking
                preview_start,
                preview_end,
                confidence,
            };

            indicators.push(indicator);
            last_indicator_time = event.timestamp;
        }

        Ok(indicators)
    }

    /// Calculate confidence that this click will become a zoom effect
    fn calculate_confidence(&self, event: &MouseEvent, all_clicks: &[&MouseEvent], index: usize) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence

        // Higher confidence for clicks that are well-spaced
        if index > 0 {
            let prev_event = all_clicks[index - 1];
            let time_gap = event.timestamp.saturating_sub(prev_event.timestamp);
            
            if time_gap > self.config.min_click_spacing * 2 {
                confidence += 0.3; // Well spaced
            } else if time_gap > self.config.min_click_spacing {
                confidence += 0.1; // Adequately spaced
            } else {
                confidence -= 0.4; // Too close together
            }
        }

        // Higher confidence for clicks near center of screen (common for demos)
        let center_distance = ((event.x - 0.5).powi(2) + (event.y - 0.5).powi(2)).sqrt();
        if center_distance < 0.3 {
            confidence += 0.2; // Near center
        }

        // Lower confidence for clicks at edges (likely UI interactions)
        if event.x < 0.05 || event.x > 0.95 || event.y < 0.05 || event.y > 0.95 {
            confidence -= 0.3; // Edge clicks
        }

        // Ensure confidence is within valid range
        confidence.clamp(0.0, 1.0)
    }
}

/// Load mouse events from a session file
fn load_mouse_events_from_session(session_path: &Path) -> Result<Vec<MouseEvent>> {
    // For now, let's assume mouse events are stored in a JSON file alongside the video
    // In the actual implementation, this would read from the session format used by the recording system
    
    let mouse_events_path = session_path.with_extension("mouse.json");
    
    if !mouse_events_path.exists() {
        // Return empty vec if no mouse data exists
        return Ok(vec![]);
    }

    let mouse_data = std::fs::read_to_string(&mouse_events_path)?;
    let mouse_events: Vec<MouseEvent> = serde_json::from_str(&mouse_data)?;
    
    Ok(mouse_events)
}

/// Tauri command to generate preview zoom indicators
#[command]
pub async fn get_preview_zoom_indicators(video_path: String) -> Result<PreviewZoomAnalysis, String> {
    let video_path = Path::new(&video_path);
    let session_path = video_path.with_extension("session");
    
    // Load mouse events from session data
    let mouse_events = load_mouse_events_from_session(&session_path)
        .map_err(|e| format!("Failed to load mouse events: {}", e))?;

    if mouse_events.is_empty() {
        // Return empty analysis if no mouse events
        return Ok(PreviewZoomAnalysis {
            indicators: vec![],
            total_clicks: 0,
            total_indicators: 0,
            session_duration: 0,
            analysis_time_ms: 0,
        });
    }

    // Calculate session duration from mouse events
    let session_duration = mouse_events.iter()
        .map(|e| e.timestamp)
        .max()
        .unwrap_or(0);

    // Generate preview indicators
    let generator = ZoomPreviewGenerator::with_default_config();
    let analysis = generator.generate_preview_indicators(&mouse_events, session_duration)
        .map_err(|e| format!("Failed to generate preview indicators: {}", e))?;

    println!("Generated {} preview zoom indicators from {} clicks in {}ms", 
        analysis.total_indicators, analysis.total_clicks, analysis.analysis_time_ms);

    Ok(analysis)
}

/// Tauri command to check if mouse data exists for preview
#[command]
pub async fn has_mouse_data_for_preview(video_path: String) -> Result<bool, String> {
    let video_path = Path::new(&video_path);
    let mouse_events_path = video_path.with_extension("mouse.json");
    Ok(mouse_events_path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_mouse_event(timestamp: u64, x: f64, y: f64) -> MouseEvent {
        MouseEvent {
            timestamp,
            x,
            y,
            event_type: MouseEventType::ButtonPress { 
                button: MouseButton::Left 
            },
            display_id: None,
        }
    }

    #[test]
    fn test_preview_indicator_generation() {
        let generator = ZoomPreviewGenerator::with_default_config();
        
        let mouse_events = vec![
            create_test_mouse_event(1000, 0.3, 0.4),  // Good click - center area
            create_test_mouse_event(1200, 0.4, 0.5),  // Too close to previous (200ms gap)
            create_test_mouse_event(2000, 0.6, 0.3),  // Good click - proper spacing (800ms gap)
            create_test_mouse_event(2100, 0.02, 0.1), // Edge click - lower confidence
        ];

        let analysis = generator.generate_preview_indicators(&mouse_events, 5000).unwrap();
        
        // Should generate indicators for first and third clicks (good spacing and confidence)
        // Second click filtered out due to spacing, fourth might be filtered by confidence
        assert!(analysis.total_indicators >= 1);
        assert!(analysis.total_indicators <= 3);
        assert_eq!(analysis.total_clicks, 4);
        assert!(analysis.analysis_time_ms < 100); // Should be fast
    }

    #[test]
    fn test_confidence_calculation() {
        let generator = ZoomPreviewGenerator::with_default_config();
        
        let center_click = create_test_mouse_event(1000, 0.5, 0.5);
        let edge_click = create_test_mouse_event(1000, 0.02, 0.02);
        
        let clicks = vec![&center_click];
        
        let center_confidence = generator.calculate_confidence(&center_click, &clicks, 0);
        let edge_confidence = generator.calculate_confidence(&edge_click, &clicks, 0);
        
        // Center clicks should have higher confidence than edge clicks
        assert!(center_confidence > edge_confidence);
        assert!(center_confidence > 0.5);
        assert!(edge_confidence < 0.5);
    }

    #[test]
    fn test_click_spacing() {
        let generator = ZoomPreviewGenerator::with_default_config();
        
        let close_clicks = vec![
            create_test_mouse_event(1000, 0.3, 0.4),  
            create_test_mouse_event(1200, 0.4, 0.5),  // 200ms gap - too close
            create_test_mouse_event(1400, 0.5, 0.6),  // 200ms gap - too close
        ];

        let analysis = generator.generate_preview_indicators(&close_clicks, 5000).unwrap();
        
        // Should only generate one indicator due to spacing requirements
        assert_eq!(analysis.total_indicators, 1);
        assert_eq!(analysis.total_clicks, 3);
    }
}