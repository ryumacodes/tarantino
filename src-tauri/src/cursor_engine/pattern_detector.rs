use anyhow::Result;
use std::collections::VecDeque;

use super::{EnhancedCursorEvent, CursorEngineConfig, DetectedPatterns};

/// **PURE ALGORITHMIC PATTERN DETECTION**
/// Detects cursor behavior patterns using deterministic rules and thresholds
pub struct PatternDetector {
    /// Configuration
    config: CursorEngineConfig,
    
    /// Pattern detection modules
    hover_detector: HoverDetector,
    click_detector: ClickDetector,
    precise_work_detector: PreciseWorkDetector,
    reading_detector: ReadingDetector,
    navigation_detector: NavigationDetector,
    
    /// Event history for pattern analysis
    event_history: VecDeque<EnhancedCursorEvent>,
    
    /// Current active patterns
    active_patterns: Vec<String>,
}

/// Detects hovering behavior
struct HoverDetector {
    hover_threshold_velocity: f64,
    min_hover_duration_ms: u64,
    hover_start_time: Option<u64>,
    is_hovering: bool,
}

/// Detects clicking patterns
struct ClickDetector {
    click_window_ms: u64,
    rapid_deceleration_threshold: f64,
    rapid_clicks: VecDeque<u64>, // timestamps
    is_clicking: bool,
}

/// Detects precise work patterns (drawing, selecting, etc.)
struct PreciseWorkDetector {
    precise_velocity_threshold: f64,
    min_precision_duration_ms: u64,
    max_velocity_variance: f64,
    precision_start_time: Option<u64>,
    velocity_samples: VecDeque<f64>,
    is_precise_work: bool,
}

/// Detects reading patterns
struct ReadingDetector {
    reading_velocity_range: (f64, f64), // min, max velocity for reading
    horizontal_bias_threshold: f64, // ratio of horizontal vs vertical movement
    line_return_detection_threshold: f64, // pixels for detecting line returns
    reading_history: VecDeque<ReadingPoint>,
    is_reading: bool,
}

#[derive(Debug, Clone)]
struct ReadingPoint {
    x: f64,
    y: f64,
    timestamp_ms: u64,
    is_horizontal_movement: bool,
}

/// Detects navigation patterns (scrolling, panning)
struct NavigationDetector {
    navigation_velocity_threshold: f64,
    min_navigation_distance: f64,
    navigation_start_time: Option<u64>,
    navigation_start_position: Option<(f64, f64)>,
    is_navigating: bool,
}

impl PatternDetector {
    pub fn new(config: &CursorEngineConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            hover_detector: HoverDetector::new(
                config.hover_velocity_threshold,
                config.hover_min_duration_ms,
            ),
            click_detector: ClickDetector::new(config.click_detection_window_ms),
            precise_work_detector: PreciseWorkDetector::new(
                config.precise_velocity_threshold,
                1000, // 1 second minimum for precision work
            ),
            reading_detector: ReadingDetector::new(),
            navigation_detector: NavigationDetector::new(config.navigation_velocity_threshold),
            event_history: VecDeque::with_capacity(config.history_size),
            active_patterns: Vec::new(),
        })
    }
    
    /// Detect patterns in cursor behavior
    pub fn detect_patterns(&mut self, event: &EnhancedCursorEvent) -> Result<DetectedPatterns> {
        // Add to history
        self.event_history.push_back(event.clone());
        if self.event_history.len() > self.config.history_size {
            self.event_history.pop_front();
        }
        
        // Run all pattern detectors
        let is_hovering = self.hover_detector.detect(event, &self.event_history)?;
        let is_clicking = self.click_detector.detect(event, &self.event_history)?;
        let is_precise_work = self.precise_work_detector.detect(event, &self.event_history)?;
        let is_reading = self.reading_detector.detect(event, &self.event_history)?;
        let is_navigating = self.navigation_detector.detect(event, &self.event_history)?;
        
        // Update active patterns list
        self.update_active_patterns(is_hovering, is_clicking, is_precise_work, is_reading, is_navigating);
        
        Ok(DetectedPatterns {
            is_hovering,
            is_clicking,
            is_precise_work,
            is_reading,
            is_navigating,
        })
    }
    
    /// Get currently active patterns
    pub fn get_active_patterns(&self) -> Vec<String> {
        self.active_patterns.clone()
    }
    
    /// Reset pattern detector
    pub fn reset(&mut self) -> Result<()> {
        self.event_history.clear();
        self.active_patterns.clear();
        self.hover_detector.reset();
        self.click_detector.reset();
        self.precise_work_detector.reset();
        self.reading_detector.reset();
        self.navigation_detector.reset();
        Ok(())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: &CursorEngineConfig) -> Result<()> {
        self.config = config.clone();
        self.hover_detector.hover_threshold_velocity = config.hover_velocity_threshold;
        self.hover_detector.min_hover_duration_ms = config.hover_min_duration_ms;
        self.click_detector.click_window_ms = config.click_detection_window_ms;
        self.precise_work_detector.precise_velocity_threshold = config.precise_velocity_threshold;
        self.navigation_detector.navigation_velocity_threshold = config.navigation_velocity_threshold;
        Ok(())
    }
    
    // Private methods
    
    fn update_active_patterns(
        &mut self,
        is_hovering: bool,
        is_clicking: bool,
        is_precise_work: bool,
        is_reading: bool,
        is_navigating: bool,
    ) {
        self.active_patterns.clear();
        
        if is_hovering { self.active_patterns.push("hovering".to_string()); }
        if is_clicking { self.active_patterns.push("clicking".to_string()); }
        if is_precise_work { self.active_patterns.push("precise_work".to_string()); }
        if is_reading { self.active_patterns.push("reading".to_string()); }
        if is_navigating { self.active_patterns.push("navigating".to_string()); }
    }
}

impl HoverDetector {
    fn new(hover_threshold_velocity: f64, min_hover_duration_ms: u64) -> Self {
        Self {
            hover_threshold_velocity,
            min_hover_duration_ms,
            hover_start_time: None,
            is_hovering: false,
        }
    }
    
    fn detect(&mut self, event: &EnhancedCursorEvent, _history: &VecDeque<EnhancedCursorEvent>) -> Result<bool> {
        // Check if velocity is below hover threshold
        if event.velocity <= self.hover_threshold_velocity {
            // Start or continue hover
            if self.hover_start_time.is_none() {
                self.hover_start_time = Some(event.timestamp_ms);
            }
            
            // Check if we've been hovering long enough
            if let Some(start_time) = self.hover_start_time {
                let hover_duration = event.timestamp_ms - start_time;
                self.is_hovering = hover_duration >= self.min_hover_duration_ms;
            }
        } else {
            // Movement detected, reset hover
            self.hover_start_time = None;
            self.is_hovering = false;
        }
        
        Ok(self.is_hovering)
    }
    
    fn reset(&mut self) {
        self.hover_start_time = None;
        self.is_hovering = false;
    }
}

impl ClickDetector {
    fn new(click_window_ms: u64) -> Self {
        Self {
            click_window_ms,
            rapid_deceleration_threshold: -500.0, // pixels/sec² - indicates sudden stops
            rapid_clicks: VecDeque::with_capacity(10),
            is_clicking: false,
        }
    }
    
    fn detect(&mut self, event: &EnhancedCursorEvent, _history: &VecDeque<EnhancedCursorEvent>) -> Result<bool> {
        // Detect rapid deceleration (indicates stopping to click)
        if event.acceleration <= self.rapid_deceleration_threshold && event.velocity > 50.0 {
            self.rapid_clicks.push_back(event.timestamp_ms);
        }
        
        // Remove old clicks outside the detection window
        while let Some(&front_time) = self.rapid_clicks.front() {
            if event.timestamp_ms - front_time > self.click_window_ms {
                self.rapid_clicks.pop_front();
            } else {
                break;
            }
        }
        
        // Consider clicking if we have rapid decelerations in the recent window
        self.is_clicking = self.rapid_clicks.len() >= 1 && 
                          event.dwell_duration_ms < 200; // Not dwelling (which would be hover)
        
        Ok(self.is_clicking)
    }
    
    fn reset(&mut self) {
        self.rapid_clicks.clear();
        self.is_clicking = false;
    }
}

impl PreciseWorkDetector {
    fn new(precise_velocity_threshold: f64, min_precision_duration_ms: u64) -> Self {
        Self {
            precise_velocity_threshold,
            min_precision_duration_ms,
            max_velocity_variance: 1000.0, // Low variance indicates consistent movement
            precision_start_time: None,
            velocity_samples: VecDeque::with_capacity(20),
            is_precise_work: false,
        }
    }
    
    fn detect(&mut self, event: &EnhancedCursorEvent, _history: &VecDeque<EnhancedCursorEvent>) -> Result<bool> {
        // Add velocity sample
        self.velocity_samples.push_back(event.velocity);
        if self.velocity_samples.len() > 20 {
            self.velocity_samples.pop_front();
        }
        
        // Check if current velocity is in precise work range
        let in_precise_range = event.velocity > 20.0 && event.velocity <= self.precise_velocity_threshold;
        
        if in_precise_range && self.velocity_samples.len() >= 10 {
            // Check velocity consistency (low variance)
            let velocity_consistency = self.calculate_velocity_consistency();
            
            if velocity_consistency > 0.7 { // High consistency
                if self.precision_start_time.is_none() {
                    self.precision_start_time = Some(event.timestamp_ms);
                }
                
                // Check duration
                if let Some(start_time) = self.precision_start_time {
                    let precision_duration = event.timestamp_ms - start_time;
                    self.is_precise_work = precision_duration >= self.min_precision_duration_ms;
                }
            } else {
                // Reset if consistency drops
                self.precision_start_time = None;
                self.is_precise_work = false;
            }
        } else {
            // Reset if out of precise range
            self.precision_start_time = None;
            self.is_precise_work = false;
        }
        
        Ok(self.is_precise_work)
    }
    
    fn calculate_velocity_consistency(&self) -> f32 {
        if self.velocity_samples.len() < 3 {
            return 0.0;
        }
        
        let velocities: Vec<f64> = self.velocity_samples.iter().copied().collect();
        let mean = velocities.iter().sum::<f64>() / velocities.len() as f64;
        
        let variance = velocities.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / velocities.len() as f64;
        
        // Convert variance to consistency score (lower variance = higher consistency)
        let consistency = 1.0 - (variance / self.max_velocity_variance).min(1.0);
        consistency as f32
    }
    
    fn reset(&mut self) {
        self.precision_start_time = None;
        self.velocity_samples.clear();
        self.is_precise_work = false;
    }
}

impl ReadingDetector {
    fn new() -> Self {
        Self {
            reading_velocity_range: (50.0, 300.0), // Typical reading speeds
            horizontal_bias_threshold: 2.0, // Horizontal movement should dominate
            line_return_detection_threshold: 100.0, // Pixels for detecting line returns
            reading_history: VecDeque::with_capacity(30),
            is_reading: false,
        }
    }
    
    fn detect(&mut self, event: &EnhancedCursorEvent, history: &VecDeque<EnhancedCursorEvent>) -> Result<bool> {
        // Add to reading history
        let is_horizontal = self.is_horizontal_movement(event, history);
        self.reading_history.push_back(ReadingPoint {
            x: event.x,
            y: event.y,
            timestamp_ms: event.timestamp_ms,
            is_horizontal_movement: is_horizontal,
        });
        
        if self.reading_history.len() > 30 {
            self.reading_history.pop_front();
        }
        
        // Analyze reading pattern
        if self.reading_history.len() >= 15 {
            let reading_score = self.calculate_reading_score();
            
            // Check if velocity is in reading range
            let velocity_in_range = event.velocity >= self.reading_velocity_range.0 && 
                                  event.velocity <= self.reading_velocity_range.1;
            
            self.is_reading = reading_score > 0.6 && velocity_in_range;
        } else {
            self.is_reading = false;
        }
        
        Ok(self.is_reading)
    }
    
    fn is_horizontal_movement(&self, event: &EnhancedCursorEvent, history: &VecDeque<EnhancedCursorEvent>) -> bool {
        if let Some(prev_event) = history.back() {
            let dx = (event.x - prev_event.x).abs();
            let dy = (event.y - prev_event.y).abs();
            
            if dy == 0.0 {
                return dx > 0.0;
            }
            
            let horizontal_ratio = dx / dy;
            horizontal_ratio >= self.horizontal_bias_threshold
        } else {
            false
        }
    }
    
    fn calculate_reading_score(&self) -> f32 {
        if self.reading_history.len() < 10 {
            return 0.0;
        }
        
        // Count horizontal movements
        let horizontal_count = self.reading_history.iter()
            .filter(|p| p.is_horizontal_movement)
            .count();
        
        let horizontal_ratio = horizontal_count as f32 / self.reading_history.len() as f32;
        
        // Detect line returns (vertical jumps after horizontal movement)
        let line_returns = self.detect_line_returns();
        let line_return_score = if self.reading_history.len() > 20 {
            (line_returns as f32 / (self.reading_history.len() / 20) as f32).min(1.0)
        } else {
            0.0
        };
        
        // Combine horizontal bias and line returns for reading score
        let reading_score = horizontal_ratio * 0.7 + line_return_score * 0.3;
        reading_score.clamp(0.0, 1.0)
    }
    
    fn detect_line_returns(&self) -> usize {
        let mut line_returns = 0;
        
        for window in self.reading_history.iter().collect::<Vec<_>>().windows(3) {
            if let [a, b, c] = window {
                // Look for pattern: horizontal movement followed by significant vertical movement
                let horizontal_movement = (b.x - a.x).abs() > 50.0 && (b.y - a.y).abs() < 20.0;
                let vertical_jump = (c.y - b.y).abs() > self.line_return_detection_threshold;
                let horizontal_reset = (c.x - a.x).abs() > 100.0; // Cursor moved to start of new line
                
                if horizontal_movement && vertical_jump && horizontal_reset {
                    line_returns += 1;
                }
            }
        }
        
        line_returns
    }
    
    fn reset(&mut self) {
        self.reading_history.clear();
        self.is_reading = false;
    }
}

impl NavigationDetector {
    fn new(navigation_velocity_threshold: f64) -> Self {
        Self {
            navigation_velocity_threshold,
            min_navigation_distance: 200.0, // pixels
            navigation_start_time: None,
            navigation_start_position: None,
            is_navigating: false,
        }
    }
    
    fn detect(&mut self, event: &EnhancedCursorEvent, _history: &VecDeque<EnhancedCursorEvent>) -> Result<bool> {
        // Check if velocity is above navigation threshold
        if event.velocity >= self.navigation_velocity_threshold {
            // Start or continue navigation
            if self.navigation_start_time.is_none() {
                self.navigation_start_time = Some(event.timestamp_ms);
                self.navigation_start_position = Some((event.x, event.y));
            }
            
            // Check if we've moved far enough to be considered navigation
            if let Some((start_x, start_y)) = self.navigation_start_position {
                let distance = ((event.x - start_x).powi(2) + (event.y - start_y).powi(2)).sqrt();
                self.is_navigating = distance >= self.min_navigation_distance;
            }
        } else {
            // Velocity dropped, check if we should continue considering this navigation
            if let Some(start_time) = self.navigation_start_time {
                let navigation_duration = event.timestamp_ms - start_time;
                
                // Stop navigation if velocity has been low for too long
                if navigation_duration > 500 { // 500ms tolerance
                    self.navigation_start_time = None;
                    self.navigation_start_position = None;
                    self.is_navigating = false;
                }
            }
        }
        
        Ok(self.is_navigating)
    }
    
    fn reset(&mut self) {
        self.navigation_start_time = None;
        self.navigation_start_position = None;
        self.is_navigating = false;
    }
}