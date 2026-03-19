#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod pattern_detector;
pub mod zoom_calculator;
pub mod motion_analyzer;

pub use pattern_detector::*;
pub use zoom_calculator::*;
pub use motion_analyzer::*;

/// **DETERMINISTIC CURSOR INTELLIGENCE ENGINE**
/// Zero AI - Pure algorithmic pattern detection and smart zoom calculations
pub struct CursorEngine {
    /// Motion analysis for velocity, acceleration, and smoothing
    motion_analyzer: MotionAnalyzer,
    
    /// Pattern detection for different cursor behaviors
    pattern_detector: PatternDetector,
    
    /// Zoom calculation engine
    zoom_calculator: ZoomCalculator,
    
    /// Configuration
    config: CursorEngineConfig,
    
    /// Performance metrics
    metrics: EngineMetrics,
}

/// Simple cursor event for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorEvent {
    pub x: f64,
    pub y: f64,
    pub timestamp_ms: u64,
}

/// Enhanced cursor event with calculated motion data
#[derive(Debug, Clone)]
pub struct EnhancedCursorEvent {
    pub x: f64,
    pub y: f64,
    pub timestamp_ms: u64,
    
    /// Calculated motion properties
    pub velocity: f64,           // pixels per second
    pub acceleration: f64,       // pixels per second squared
    pub direction: f64,          // angle in radians
    pub distance_from_prev: f64, // pixels
    
    /// Detected patterns
    pub is_hovering: bool,
    pub is_clicking: bool,
    pub is_precise_work: bool,
    pub is_reading: bool,
    pub is_navigating: bool,
    
    /// Dwell and stability metrics
    pub dwell_duration_ms: u64,
    pub stability_score: f32, // 0.0 - 1.0, higher = more stable
}

/// Zoom recommendation with all parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomRecommendation {
    /// When to start zooming
    pub start_time_ms: u64,
    /// When to end zooming  
    pub end_time_ms: u64,
    /// Focus point (normalized 0.0-1.0)
    pub focus_x: f32,
    pub focus_y: f32,
    /// Zoom level (1.0 = no zoom)
    pub zoom_factor: f32,
    /// Confidence in this recommendation
    pub confidence: f32,
    /// Reason for zooming
    pub reason: ZoomReason,
    /// Animation curve
    pub easing: ZoomEasing,
}

/// Reasons for zoom recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoomReason {
    PreciseWork,
    ClickActivity, 
    HoverFocus,
    ReadingText,
    NavigationEnd,
    ManualTrigger,
}

/// Zoom animation curves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoomEasing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
}

/// Configuration for the cursor engine
#[derive(Debug, Clone)]
pub struct CursorEngineConfig {
    /// Screen dimensions for normalization
    pub screen_width: f32,
    pub screen_height: f32,
    
    /// Motion thresholds
    pub hover_velocity_threshold: f64,    // pixels/sec - below this is hovering
    pub precise_velocity_threshold: f64,   // pixels/sec - below this is precise work
    pub navigation_velocity_threshold: f64, // pixels/sec - above this is navigation
    
    /// Time thresholds
    pub hover_min_duration_ms: u64,       // minimum time to be considered hovering
    pub click_detection_window_ms: u64,   // window for detecting click patterns
    pub zoom_cooldown_ms: u64,            // minimum time between zoom recommendations
    
    /// Zoom parameters
    pub min_zoom_factor: f32,
    pub max_zoom_factor: f32,
    pub default_zoom_duration_ms: u64,
    
    /// Pattern detection sensitivity (0.0 - 1.0)
    pub pattern_sensitivity: f32,
    
    /// Event history size
    pub history_size: usize,
}

impl Default for CursorEngineConfig {
    fn default() -> Self {
        Self {
            screen_width: 1920.0,
            screen_height: 1080.0,
            hover_velocity_threshold: 20.0,
            precise_velocity_threshold: 80.0,
            navigation_velocity_threshold: 300.0,
            hover_min_duration_ms: 500,
            click_detection_window_ms: 200,
            zoom_cooldown_ms: 1000,
            min_zoom_factor: 1.1,
            max_zoom_factor: 3.0,
            default_zoom_duration_ms: 3000,
            pattern_sensitivity: 0.7,
            history_size: 100,
        }
    }
}

/// Performance and usage metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineMetrics {
    pub events_processed: u64,
    pub zoom_recommendations: u64,
    pub pattern_detections: PatternMetrics,
    pub performance: PerformanceMetrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternMetrics {
    pub hover_detections: u32,
    pub click_detections: u32,
    pub precise_work_detections: u32,
    pub reading_detections: u32,
    pub navigation_detections: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_processing_time_us: u64,
    pub max_processing_time_us: u64,
    pub total_processing_time_us: u64,
}

impl CursorEngine {
    /// Create new cursor engine with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(CursorEngineConfig::default())
    }
    
    /// Create new cursor engine with custom configuration
    pub fn with_config(config: CursorEngineConfig) -> Result<Self> {
        let motion_analyzer = MotionAnalyzer::new(&config)?;
        let pattern_detector = PatternDetector::new(&config)?;
        let zoom_calculator = ZoomCalculator::new(&config)?;
        
        Ok(Self {
            motion_analyzer,
            pattern_detector,
            zoom_calculator,
            config,
            metrics: EngineMetrics::default(),
        })
    }
    
    /// Process a cursor event and generate zoom recommendations
    pub fn process_event(&mut self, event: CursorEvent) -> Result<Vec<ZoomRecommendation>> {
        let start_time = SystemTime::now();
        
        // 1. Enhance event with motion analysis
        let enhanced_event = self.motion_analyzer.analyze_motion(event)?;
        
        // 2. Detect patterns in cursor behavior
        let patterns = self.pattern_detector.detect_patterns(&enhanced_event)?;
        
        // 3. Update enhanced event with pattern information
        let mut enhanced_with_patterns = enhanced_event;
        enhanced_with_patterns.is_hovering = patterns.is_hovering;
        enhanced_with_patterns.is_clicking = patterns.is_clicking;
        enhanced_with_patterns.is_precise_work = patterns.is_precise_work;
        enhanced_with_patterns.is_reading = patterns.is_reading;
        enhanced_with_patterns.is_navigating = patterns.is_navigating;
        
        // 4. Calculate zoom recommendations
        let zoom_recommendations = self.zoom_calculator.calculate_zoom(&enhanced_with_patterns)?;
        
        // 5. Update metrics
        self.update_metrics(&enhanced_with_patterns, &zoom_recommendations, start_time)?;
        
        Ok(zoom_recommendations)
    }
    
    /// Get current engine metrics
    pub fn get_metrics(&self) -> &EngineMetrics {
        &self.metrics
    }
    
    /// Reset engine state (for new recording session)
    pub fn reset(&mut self) -> Result<()> {
        self.motion_analyzer.reset()?;
        self.pattern_detector.reset()?;
        self.zoom_calculator.reset()?;
        self.metrics = EngineMetrics::default();
        Ok(())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: CursorEngineConfig) -> Result<()> {
        self.config = config.clone();
        self.motion_analyzer.update_config(&config)?;
        self.pattern_detector.update_config(&config)?;
        self.zoom_calculator.update_config(&config)?;
        Ok(())
    }
    
    /// Get current cursor state
    pub fn get_current_state(&self) -> Result<CursorState> {
        Ok(CursorState {
            last_position: self.motion_analyzer.get_last_position(),
            current_velocity: self.motion_analyzer.get_current_velocity(),
            is_stable: self.motion_analyzer.is_stable(),
            active_patterns: self.pattern_detector.get_active_patterns(),
            zoom_active: self.zoom_calculator.has_active_zoom(),
            last_update_ms: self.get_current_time_ms(),
        })
    }
    
    // Private helper methods
    
    fn update_metrics(
        &mut self,
        event: &EnhancedCursorEvent,
        recommendations: &[ZoomRecommendation],
        start_time: SystemTime,
    ) -> Result<()> {
        // Update processing metrics
        let processing_time_us = start_time.elapsed()?.as_micros() as u64;
        
        self.metrics.events_processed += 1;
        self.metrics.zoom_recommendations += recommendations.len() as u64;
        self.metrics.performance.total_processing_time_us += processing_time_us;
        self.metrics.performance.avg_processing_time_us = 
            self.metrics.performance.total_processing_time_us / self.metrics.events_processed;
        
        if processing_time_us > self.metrics.performance.max_processing_time_us {
            self.metrics.performance.max_processing_time_us = processing_time_us;
        }
        
        // Update pattern metrics
        if event.is_hovering {
            self.metrics.pattern_detections.hover_detections += 1;
        }
        if event.is_clicking {
            self.metrics.pattern_detections.click_detections += 1;
        }
        if event.is_precise_work {
            self.metrics.pattern_detections.precise_work_detections += 1;
        }
        if event.is_reading {
            self.metrics.pattern_detections.reading_detections += 1;
        }
        if event.is_navigating {
            self.metrics.pattern_detections.navigation_detections += 1;
        }
        
        Ok(())
    }
    
    fn get_current_time_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u64
    }
}

/// Current state of the cursor engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    pub last_position: (f64, f64),
    pub current_velocity: f64,
    pub is_stable: bool,
    pub active_patterns: Vec<String>,
    pub zoom_active: bool,
    pub last_update_ms: u64,
}

/// Detected cursor patterns
#[derive(Debug, Clone, Default)]
pub struct DetectedPatterns {
    pub is_hovering: bool,
    pub is_clicking: bool,
    pub is_precise_work: bool,
    pub is_reading: bool,
    pub is_navigating: bool,
}

/// Export function to create cursor engine with default settings
pub fn create_cursor_engine() -> Result<CursorEngine> {
    CursorEngine::new()
}

/// Export function to create cursor engine with custom screen dimensions
pub fn create_cursor_engine_for_screen(width: f32, height: f32) -> Result<CursorEngine> {
    let mut config = CursorEngineConfig::default();
    config.screen_width = width;
    config.screen_height = height;
    CursorEngine::with_config(config)
}