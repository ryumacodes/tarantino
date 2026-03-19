use anyhow::Result;
use std::collections::VecDeque;
use std::f64::consts::PI;

use super::{CursorEvent, EnhancedCursorEvent, CursorEngineConfig};

/// **PURE ALGORITHMIC MOTION ANALYSIS**
/// Calculates velocity, acceleration, direction, and stability using deterministic math
pub struct MotionAnalyzer {
    /// Recent cursor positions for analysis
    position_history: VecDeque<MotionPoint>,
    
    /// Configuration
    config: CursorEngineConfig,
    
    /// Current motion state
    current_velocity: f64,
    current_acceleration: f64,
    current_direction: f64,
    
    /// Smoothing filters
    velocity_smoother: ExponentialSmoother,
    acceleration_smoother: ExponentialSmoother,
    
    /// Stability analysis
    stability_analyzer: StabilityAnalyzer,
}

/// Point in motion analysis
#[derive(Debug, Clone)]
struct MotionPoint {
    x: f64,
    y: f64,
    timestamp_ms: u64,
    
    // Calculated properties
    velocity: f64,
    acceleration: f64,
    direction: f64,
    distance_from_prev: f64,
}

/// Exponential smoothing filter for noise reduction
#[derive(Debug, Clone)]
struct ExponentialSmoother {
    alpha: f64,      // Smoothing factor (0.0 - 1.0)
    last_value: f64, // Previous smoothed value
    initialized: bool,
}

/// Analyzes cursor stability and dwelling
#[derive(Debug)]
struct StabilityAnalyzer {
    /// Points within stability threshold
    stable_points: VecDeque<MotionPoint>,
    
    /// Current stability metrics
    stability_score: f32,
    dwell_start_time: Option<u64>,
    dwell_position: Option<(f64, f64)>,
    
    /// Stability parameters
    stability_radius: f64, // pixels - radius for considering points "stable"
    min_stable_duration: u64, // ms - minimum time to be considered dwelling
}

impl MotionAnalyzer {
    pub fn new(config: &CursorEngineConfig) -> Result<Self> {
        Ok(Self {
            position_history: VecDeque::with_capacity(config.history_size),
            config: config.clone(),
            current_velocity: 0.0,
            current_acceleration: 0.0,
            current_direction: 0.0,
            velocity_smoother: ExponentialSmoother::new(0.3), // 30% smoothing
            acceleration_smoother: ExponentialSmoother::new(0.5), // 50% smoothing
            stability_analyzer: StabilityAnalyzer::new(30.0, config.hover_min_duration_ms),
        })
    }
    
    /// Analyze motion and enhance cursor event
    pub fn analyze_motion(&mut self, event: CursorEvent) -> Result<EnhancedCursorEvent> {
        // Calculate motion properties
        let (velocity, acceleration, direction, distance) = 
            self.calculate_motion_properties(&event)?;
        
        // Apply smoothing
        let smoothed_velocity = self.velocity_smoother.smooth(velocity);
        let smoothed_acceleration = self.acceleration_smoother.smooth(acceleration);
        
        // Update current state
        self.current_velocity = smoothed_velocity;
        self.current_acceleration = smoothed_acceleration;
        self.current_direction = direction;
        
        // Create motion point
        let motion_point = MotionPoint {
            x: event.x,
            y: event.y,
            timestamp_ms: event.timestamp_ms,
            velocity: smoothed_velocity,
            acceleration: smoothed_acceleration,
            direction,
            distance_from_prev: distance,
        };
        
        // Add to history
        self.position_history.push_back(motion_point.clone());
        if self.position_history.len() > self.config.history_size {
            self.position_history.pop_front();
        }
        
        // Analyze stability
        let (stability_score, dwell_duration_ms) = 
            self.stability_analyzer.analyze_stability(&motion_point)?;
        
        // Create enhanced event
        Ok(EnhancedCursorEvent {
            x: event.x,
            y: event.y,
            timestamp_ms: event.timestamp_ms,
            velocity: smoothed_velocity,
            acceleration: smoothed_acceleration,
            direction,
            distance_from_prev: distance,
            
            // Pattern flags (will be set by PatternDetector)
            is_hovering: false,
            is_clicking: false,
            is_precise_work: false,
            is_reading: false,
            is_navigating: false,
            
            // Stability metrics
            dwell_duration_ms,
            stability_score,
        })
    }
    
    /// Get current velocity
    pub fn get_current_velocity(&self) -> f64 {
        self.current_velocity
    }
    
    /// Get last position
    pub fn get_last_position(&self) -> (f64, f64) {
        self.position_history
            .back()
            .map(|p| (p.x, p.y))
            .unwrap_or((0.0, 0.0))
    }
    
    /// Check if cursor is currently stable
    pub fn is_stable(&self) -> bool {
        self.stability_analyzer.stability_score > 0.7
    }
    
    /// Reset motion analyzer
    pub fn reset(&mut self) -> Result<()> {
        self.position_history.clear();
        self.current_velocity = 0.0;
        self.current_acceleration = 0.0;
        self.current_direction = 0.0;
        self.velocity_smoother.reset();
        self.acceleration_smoother.reset();
        self.stability_analyzer.reset();
        Ok(())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: &CursorEngineConfig) -> Result<()> {
        self.config = config.clone();
        self.stability_analyzer.min_stable_duration = config.hover_min_duration_ms;
        Ok(())
    }
    
    /// Get motion statistics for analysis
    pub fn get_motion_stats(&self) -> MotionStats {
        if self.position_history.len() < 2 {
            return MotionStats::default();
        }
        
        let velocities: Vec<f64> = self.position_history.iter().map(|p| p.velocity).collect();
        let accelerations: Vec<f64> = self.position_history.iter().map(|p| p.acceleration).collect();
        
        let avg_velocity = velocities.iter().sum::<f64>() / velocities.len() as f64;
        let max_velocity = velocities.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_velocity = velocities.iter().fold(f64::MAX, |a, &b| a.min(b));
        
        let avg_acceleration = accelerations.iter().sum::<f64>() / accelerations.len() as f64;
        let max_acceleration = accelerations.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_acceleration = accelerations.iter().fold(f64::MAX, |a, &b| a.min(b));
        
        // Calculate velocity variance for smoothness metrics
        let velocity_variance = velocities.iter()
            .map(|v| (v - avg_velocity).powi(2))
            .sum::<f64>() / velocities.len() as f64;
        
        MotionStats {
            avg_velocity,
            max_velocity,
            min_velocity,
            avg_acceleration,
            max_acceleration,
            min_acceleration,
            velocity_variance,
            total_distance: self.calculate_total_distance(),
            motion_smoothness: self.calculate_motion_smoothness(),
        }
    }
    
    // Private implementation methods
    
    fn calculate_motion_properties(&self, event: &CursorEvent) -> Result<(f64, f64, f64, f64)> {
        if self.position_history.is_empty() {
            return Ok((0.0, 0.0, 0.0, 0.0));
        }
        
        let last_point = self.position_history.back().unwrap();
        
        // Calculate distance and direction
        let dx = event.x - last_point.x;
        let dy = event.y - last_point.y;
        let distance = (dx * dx + dy * dy).sqrt();
        let direction = dy.atan2(dx); // angle in radians
        
        // Calculate time difference
        let time_diff_ms = event.timestamp_ms - last_point.timestamp_ms;
        if time_diff_ms == 0 {
            return Ok((0.0, 0.0, direction, distance));
        }
        
        let time_diff_sec = time_diff_ms as f64 / 1000.0;
        
        // Calculate velocity (pixels per second)
        let velocity = distance / time_diff_sec;
        
        // Calculate acceleration (change in velocity)
        let acceleration = if self.position_history.len() >= 2 {
            (velocity - last_point.velocity) / time_diff_sec
        } else {
            0.0
        };
        
        Ok((velocity, acceleration, direction, distance))
    }
    
    fn calculate_total_distance(&self) -> f64 {
        self.position_history.iter()
            .map(|p| p.distance_from_prev)
            .sum()
    }
    
    fn calculate_motion_smoothness(&self) -> f32 {
        if self.position_history.len() < 3 {
            return 1.0;
        }
        
        // Calculate smoothness based on direction changes
        let mut direction_changes = 0;
        let mut total_segments = 0;
        
        for window in self.position_history.iter().collect::<Vec<_>>().windows(3) {
            if let [a, b, c] = window {
                let angle1 = a.direction;
                let angle2 = b.direction;
                let angle3 = c.direction;
                
                // Calculate angle differences (normalized)
                let diff1 = self.normalize_angle_diff(angle2 - angle1);
                let diff2 = self.normalize_angle_diff(angle3 - angle2);
                
                // If direction change is significant, count it
                if diff1.abs() > PI / 6.0 || diff2.abs() > PI / 6.0 {
                    direction_changes += 1;
                }
                total_segments += 1;
            }
        }
        
        if total_segments > 0 {
            1.0 - (direction_changes as f32 / total_segments as f32)
        } else {
            1.0
        }
    }
    
    fn normalize_angle_diff(&self, angle_diff: f64) -> f64 {
        let mut diff = angle_diff;
        while diff > PI {
            diff -= 2.0 * PI;
        }
        while diff <= -PI {
            diff += 2.0 * PI;
        }
        diff
    }
}

impl ExponentialSmoother {
    fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            last_value: 0.0,
            initialized: false,
        }
    }
    
    fn smooth(&mut self, value: f64) -> f64 {
        if !self.initialized {
            self.last_value = value;
            self.initialized = true;
            return value;
        }
        
        // Exponential smoothing formula: S_t = α * X_t + (1 - α) * S_(t-1)
        let smoothed = self.alpha * value + (1.0 - self.alpha) * self.last_value;
        self.last_value = smoothed;
        smoothed
    }
    
    fn reset(&mut self) {
        self.last_value = 0.0;
        self.initialized = false;
    }
}

impl StabilityAnalyzer {
    fn new(stability_radius: f64, min_stable_duration: u64) -> Self {
        Self {
            stable_points: VecDeque::with_capacity(50),
            stability_score: 0.0,
            dwell_start_time: None,
            dwell_position: None,
            stability_radius,
            min_stable_duration,
        }
    }
    
    fn analyze_stability(&mut self, point: &MotionPoint) -> Result<(f32, u64)> {
        // Check if this point is within stability radius of previous points
        let is_stable_point = self.is_point_stable(point);
        
        if is_stable_point {
            // Add to stable points
            self.stable_points.push_back(point.clone());
            if self.stable_points.len() > 20 {
                self.stable_points.pop_front();
            }
            
            // Update dwell tracking
            if self.dwell_start_time.is_none() {
                self.dwell_start_time = Some(point.timestamp_ms);
                self.dwell_position = Some((point.x, point.y));
            }
            
            // Calculate stability score based on consistency
            self.stability_score = self.calculate_stability_score();
        } else {
            // Reset dwell tracking if we moved significantly
            self.dwell_start_time = None;
            self.dwell_position = None;
            self.stable_points.clear();
            self.stability_score = 0.0;
        }
        
        // Calculate dwell duration
        let dwell_duration_ms = if let Some(start_time) = self.dwell_start_time {
            point.timestamp_ms - start_time
        } else {
            0
        };
        
        Ok((self.stability_score, dwell_duration_ms))
    }
    
    fn is_point_stable(&self, point: &MotionPoint) -> bool {
        // If no dwell position established, use velocity threshold
        if self.dwell_position.is_none() {
            return point.velocity < 30.0; // Low velocity indicates potential stability
        }
        
        // Check distance from dwell position
        if let Some((dwell_x, dwell_y)) = self.dwell_position {
            let distance = ((point.x - dwell_x).powi(2) + (point.y - dwell_y).powi(2)).sqrt();
            distance <= self.stability_radius
        } else {
            false
        }
    }
    
    fn calculate_stability_score(&self) -> f32 {
        if self.stable_points.len() < 3 {
            return 0.0;
        }
        
        // Calculate variance in positions
        let positions: Vec<(f64, f64)> = self.stable_points.iter()
            .map(|p| (p.x, p.y))
            .collect();
        
        let mean_x = positions.iter().map(|(x, _)| *x).sum::<f64>() / positions.len() as f64;
        let mean_y = positions.iter().map(|(_, y)| *y).sum::<f64>() / positions.len() as f64;
        
        let variance = positions.iter()
            .map(|(x, y)| (x - mean_x).powi(2) + (y - mean_y).powi(2))
            .sum::<f64>() / positions.len() as f64;
        
        // Convert variance to stability score (lower variance = higher stability)
        let max_expected_variance = self.stability_radius * self.stability_radius;
        let stability = 1.0 - (variance / max_expected_variance).min(1.0);
        
        stability as f32
    }
    
    fn reset(&mut self) {
        self.stable_points.clear();
        self.stability_score = 0.0;
        self.dwell_start_time = None;
        self.dwell_position = None;
    }
}

/// Motion statistics for analysis
#[derive(Debug, Clone, Default)]
pub struct MotionStats {
    pub avg_velocity: f64,
    pub max_velocity: f64,
    pub min_velocity: f64,
    pub avg_acceleration: f64,
    pub max_acceleration: f64,
    pub min_acceleration: f64,
    pub velocity_variance: f64,
    pub total_distance: f64,
    pub motion_smoothness: f32, // 0.0 - 1.0, higher = smoother
}