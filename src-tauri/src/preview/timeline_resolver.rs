use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::capture::{Project, Clip, CursorEvent};
use super::{ResolvedFrameData, CursorData, ZoomData, ProjectLoader};

/// Timeline resolver that maps project time to clip-local time and resolves frame data
/// 
/// This handles the segment-based structure of projects where clips can be split
/// across multiple files due to Studio mode breaks. It maps global project time
/// to the correct clip and local time within that clip.
pub struct TimelineResolver {
    /// Currently loaded project
    current_project: Arc<Mutex<Option<Project>>>,
    
    /// Cached clip lookup for performance
    clip_boundaries: Arc<Mutex<Vec<ClipBoundary>>>,
    
    /// Project loader reference for getting frame data
    project_loader: Arc<Mutex<Option<Arc<Mutex<ProjectLoader>>>>>,
}

#[derive(Debug, Clone)]
struct ClipBoundary {
    clip_id: String,
    start_time: u64,  // Start time in project timeline
    end_time: u64,    // End time in project timeline
    duration: u64,    // Clip duration
}

impl TimelineResolver {
    pub fn new() -> Self {
        Self {
            current_project: Arc::new(Mutex::new(None)),
            clip_boundaries: Arc::new(Mutex::new(Vec::new())),
            project_loader: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Set the current project and build clip boundaries
    pub async fn set_project(&self, project: &Project) -> Result<()> {
        println!("Timeline resolver loading project: {}", project.id);
        
        // Store project
        {
            let mut current_project = self.current_project.lock().await;
            *current_project = Some(project.clone());
        }
        
        // Build clip boundaries for efficient lookups
        self.build_clip_boundaries(project).await?;
        
        println!("Timeline resolver loaded: {} clips", project.clips.len());
        Ok(())
    }
    
    /// Set the project loader reference
    pub async fn set_project_loader(&self, loader: Arc<Mutex<ProjectLoader>>) {
        let mut project_loader = self.project_loader.lock().await;
        *project_loader = Some(loader);
    }
    
    /// Resolve frame data for a specific project time
    pub async fn resolve_frame(&self, project_time_ms: u64) -> Result<ResolvedFrameData> {
        // Find which clip contains this timestamp
        let clip_info = self.find_clip_at_time(project_time_ms).await;

        let (_clip_id, local_time) = if let Some((clip_id, local_time)) = clip_info {
            (clip_id, local_time)
        } else {
            // Time is outside any clip - return empty frame
            return Ok(ResolvedFrameData {
                display_frame: None,
                camera_frame: None,
                cursor_data: None,
                zoom_data: None,
                timestamp_ms: project_time_ms,
            });
        };
        
        // Get project loader
        let project_loader = {
            let loader_guard = self.project_loader.lock().await;
            loader_guard.as_ref().cloned()
        };
        
        if let Some(loader) = project_loader {
            // Resolve display frame
            let display_frame = {
                let loader = loader.lock().await;
                loader.get_video_frame("display", 0, local_time).await?
            };
            
            // Resolve camera frame (if camera tracks exist)
            let camera_frame = {
                let loader = loader.lock().await;
                loader.get_video_frame("camera", 0, local_time).await?
            };
            
            // Resolve cursor data
            let cursor_data = self.resolve_cursor_at_time(project_time_ms).await?;
            
            // Resolve zoom data
            let zoom_data = self.resolve_zoom_at_time(project_time_ms).await?;
            
            Ok(ResolvedFrameData {
                display_frame,
                camera_frame,
                cursor_data,
                zoom_data,
                timestamp_ms: project_time_ms,
            })
        } else {
            Err(anyhow::anyhow!("No project loader set in timeline resolver"))
        }
    }
    
    /// Get total project duration
    pub async fn get_total_duration(&self) -> u64 {
        let boundaries = self.clip_boundaries.lock().await;
        boundaries.iter().map(|b| b.end_time).max().unwrap_or(0)
    }
    
    /// Get all clips in the project
    pub async fn get_clips(&self) -> Vec<Clip> {
        let project = self.current_project.lock().await;
        if let Some(project) = project.as_ref() {
            project.clips.clone()
        } else {
            Vec::new()
        }
    }
    
    /// Find which clip contains a specific project time
    async fn find_clip_at_time(&self, project_time_ms: u64) -> Option<(String, u64)> {
        let boundaries = self.clip_boundaries.lock().await;
        
        for boundary in boundaries.iter() {
            if project_time_ms >= boundary.start_time && project_time_ms < boundary.end_time {
                // Calculate local time within the clip
                let local_time = project_time_ms - boundary.start_time;
                return Some((boundary.clip_id.clone(), local_time));
            }
        }
        
        None
    }
    
    /// Build clip boundaries from project clips
    async fn build_clip_boundaries(&self, project: &Project) -> Result<()> {
        let mut boundaries = Vec::new();
        let mut current_time = 0u64;
        
        for clip in &project.clips {
            let boundary = ClipBoundary {
                clip_id: clip.id.clone(),
                start_time: current_time,
                end_time: current_time + clip.duration,
                duration: clip.duration,
            };
            
            boundaries.push(boundary);
            current_time += clip.duration;
        }
        
        let mut clip_boundaries = self.clip_boundaries.lock().await;
        *clip_boundaries = boundaries;
        
        Ok(())
    }
    
    /// Resolve cursor data at a specific time
    async fn resolve_cursor_at_time(&self, project_time_ms: u64) -> Result<Option<CursorData>> {
        let project = self.current_project.lock().await;
        if let Some(project) = project.as_ref() {
            // Find cursor events near this timestamp
            let cursor_events = &project.cursor_events;
            
            // Find the most recent cursor event at or before this time
            let mut latest_event: Option<&CursorEvent> = None;
            for event in cursor_events {
                if event.t <= project_time_ms {
                    latest_event = Some(event);
                } else {
                    break; // Events are sorted by time
                }
            }
            
            if let Some(event) = latest_event {
                // Convert cursor event to cursor data for rendering
                Ok(Some(CursorData {
                    x: event.x as f32,
                    y: event.y as f32,
                    width: 32,  // Default cursor size
                    height: 32,
                    image: self.get_default_cursor_image(),
                    visible: true,
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    /// Resolve zoom effects at a specific time
    async fn resolve_zoom_at_time(&self, project_time_ms: u64) -> Result<Option<ZoomData>> {
        let project = self.current_project.lock().await;
        if let Some(project) = project.as_ref() {
            // Find active zoom segments at this time
            for zoom_segment in &project.effects.zoom {
                if project_time_ms >= zoom_segment.start && project_time_ms <= zoom_segment.end {
                    // Calculate progress through the zoom segment
                    let segment_duration = zoom_segment.end - zoom_segment.start;
                    let elapsed = project_time_ms - zoom_segment.start;
                    let progress = if segment_duration > 0 {
                        elapsed as f32 / segment_duration as f32
                    } else {
                        1.0
                    };
                    
                    // Determine focus point based on zoom mode
                    let (focus_x, focus_y) = match &zoom_segment.mode {
                        crate::capture::ZoomMode::Auto => {
                            // For auto zoom, focus follows cursor position
                            self.get_cursor_focus_point(project_time_ms).await
                        }
                        crate::capture::ZoomMode::Spot { x, y } => (*x as f32, *y as f32),
                    };
                    
                    return Ok(Some(ZoomData {
                        focus_x,
                        focus_y,
                        zoom_factor: zoom_segment.strength as f32,
                        progress: progress.clamp(0.0, 1.0),
                    }));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Get cursor focus point for auto zoom mode
    async fn get_cursor_focus_point(&self, project_time_ms: u64) -> (f32, f32) {
        let project = self.current_project.lock().await;
        if let Some(project) = project.as_ref() {
            // Find cursor position at this time (same logic as resolve_cursor_at_time)
            for event in &project.cursor_events {
                if event.t <= project_time_ms {
                    if let Some(next_event) = project.cursor_events.iter().find(|e| e.t > project_time_ms) {
                        // Interpolate between current and next event for smooth movement
                        let time_diff = next_event.t - event.t;
                        if time_diff > 0 {
                            let t = (project_time_ms - event.t) as f32 / time_diff as f32;
                            let x = event.x as f32 + t * (next_event.x as f32 - event.x as f32);
                            let y = event.y as f32 + t * (next_event.y as f32 - event.y as f32);
                            return (x, y);
                        }
                    }
                    return (event.x as f32, event.y as f32);
                }
            }
        }
        
        // Default to center if no cursor data
        (0.5, 0.5)
    }
    
    /// Get default cursor image (32x32 RGBA)
    fn get_default_cursor_image(&self) -> Vec<u8> {
        // Create a simple white arrow cursor
        let mut image = vec![0u8; 32 * 32 * 4];
        
        // Draw a simple arrow shape
        for y in 0..32 {
            for x in 0..32 {
                let idx = (y * 32 + x) * 4;
                
                // Simple arrow pattern
                if (x < 16 && y < 16 && x <= y) || 
                   (x < 8 && y >= 16 && y < 24) ||
                   (y >= 16 && y < 20 && x >= 8 && x < 16) {
                    // White pixel with alpha
                    image[idx] = 255;     // R
                    image[idx + 1] = 255; // G  
                    image[idx + 2] = 255; // B
                    image[idx + 3] = 255; // A
                }
                // Else remains transparent (0, 0, 0, 0)
            }
        }
        
        image
    }
}