use anyhow::Result;
use std::collections::VecDeque;

use super::{EnhancedCursorEvent, CursorEngineConfig, ZoomRecommendation, ZoomReason, ZoomEasing};

/// **DETERMINISTIC ZOOM CALCULATION ENGINE**
/// Makes intelligent zoom decisions using pure algorithms and mathematical rules
pub struct ZoomCalculator {
    /// Configuration
    config: CursorEngineConfig,
    
    /// Zoom calculation rules
    zoom_rules: Vec<ZoomRule>,
    
    /// Active zoom sessions
    active_zooms: VecDeque<ActiveZoom>,
    
    /// Cooldown management
    last_recommendation_time: u64,
    
    /// Zoom optimization
    focus_smoother: FocusSmoother,
    
    /// Statistics
    zoom_stats: ZoomStats,
}

/// Rule for determining zoom behavior
#[derive(Debug, Clone)]
struct ZoomRule {
    name: String,
    priority: u32,
    conditions: ZoomConditions,
    zoom_action: ZoomAction,
}

/// Conditions that trigger zoom
#[derive(Debug, Clone)]
struct ZoomConditions {
    // Pattern requirements
    requires_hovering: bool,
    requires_clicking: bool,
    requires_precise_work: bool,
    requires_reading: bool,
    requires_navigating: bool,
    
    // Motion requirements
    max_velocity: Option<f64>,
    min_velocity: Option<f64>,
    max_acceleration: Option<f64>,
    min_stability_score: Option<f32>,
    min_dwell_duration_ms: Option<u64>,
    
    // Context requirements
    min_distance_from_edge: Option<f64>, // pixels from screen edge
}

/// Action to take when rule triggers
#[derive(Debug, Clone)]
struct ZoomAction {
    zoom_factor: f32,
    duration_ms: u64,
    easing: ZoomEasing,
    reason: ZoomReason,
    confidence: f32,
    
    // Focus point calculation
    focus_strategy: FocusStrategy,
    
    // Priority handling
    can_interrupt_existing: bool,
}

/// Strategy for calculating zoom focus point
#[derive(Debug, Clone)]
enum FocusStrategy {
    /// Focus on current cursor position
    Cursor,
    /// Focus on predicted cursor position
    PredictedCursor,
    /// Focus on center of recent activity
    ActivityCenter,
    /// Focus on optimal point for detected pattern
    PatternOptimal,
}

/// Active zoom session
#[derive(Debug, Clone)]
struct ActiveZoom {
    id: String,
    start_time_ms: u64,
    end_time_ms: u64,
    focus_x: f32,
    focus_y: f32,
    zoom_factor: f32,
    reason: ZoomReason,
    priority: u32,
}

/// Smooths focus point calculations to prevent jittery movement
struct FocusSmoother {
    last_focus: Option<(f32, f32)>,
    smoothing_factor: f32,
    prediction_weight: f32,
}

/// Zoom calculation statistics
#[derive(Debug, Clone, Default)]
pub struct ZoomStats {
    total_calculations: u64,
    successful_zooms: u64,
    zoom_by_reason: std::collections::HashMap<String, u32>,
    avg_zoom_factor: f32,
    avg_zoom_duration_ms: u64,
}

impl ZoomCalculator {
    pub fn new(config: &CursorEngineConfig) -> Result<Self> {
        let zoom_rules = Self::create_zoom_rules(config)?;
        
        Ok(Self {
            config: config.clone(),
            zoom_rules,
            active_zooms: VecDeque::with_capacity(5), // Max 5 concurrent zooms
            last_recommendation_time: 0,
            focus_smoother: FocusSmoother::new(0.3, 0.2),
            zoom_stats: ZoomStats::default(),
        })
    }
    
    /// Calculate zoom recommendations based on cursor behavior
    pub fn calculate_zoom(&mut self, event: &EnhancedCursorEvent) -> Result<Vec<ZoomRecommendation>> {
        self.zoom_stats.total_calculations += 1;
        
        // Clean up expired zooms
        self.cleanup_expired_zooms(event.timestamp_ms);
        
        // Check cooldown
        if !self.can_recommend_zoom(event.timestamp_ms) {
            return Ok(Vec::new());
        }
        
        // Evaluate zoom rules in priority order - clone to avoid borrowing issues
        let rules = self.zoom_rules.clone();
        let mut best_rule: Option<ZoomRule> = None;
        let mut best_score = 0.0f32;
        
        for rule in &rules {
            if let Some(score) = self.evaluate_rule(rule, event)? {
                if score > best_score && score > 0.6 { // Minimum confidence threshold
                    best_rule = Some(rule.clone());
                    best_score = score;
                }
            }
        }
        
        // Generate recommendation from best rule
        if let Some(rule) = best_rule {
            let recommendation = self.create_recommendation(&rule, event, best_score)?;
            
            // Check if this zoom should interrupt existing zooms
            if rule.zoom_action.can_interrupt_existing {
                self.clear_lower_priority_zooms(rule.priority);
            } else if self.has_conflicting_zoom(&recommendation) {
                return Ok(Vec::new()); // Don't recommend if would conflict
            }
            
            // Create active zoom session
            self.create_active_zoom(&recommendation, rule.priority);
            
            // Update stats
            self.update_zoom_stats(&recommendation);
            
            // Update last recommendation time
            self.last_recommendation_time = event.timestamp_ms;
            
            Ok(vec![recommendation])
        } else {
            Ok(Vec::new())
        }
    }
    
    /// Check if zoom is currently active
    pub fn has_active_zoom(&self) -> bool {
        !self.active_zooms.is_empty()
    }
    
    /// Get current zoom state
    pub fn get_zoom_state(&self, current_time_ms: u64) -> Option<ZoomState> {
        // Find highest priority active zoom
        let active_zoom = self.active_zooms.iter()
            .filter(|zoom| current_time_ms >= zoom.start_time_ms && current_time_ms <= zoom.end_time_ms)
            .max_by_key(|zoom| zoom.priority)?;
        
        // Calculate current zoom progress
        let total_duration = active_zoom.end_time_ms - active_zoom.start_time_ms;
        let elapsed = current_time_ms - active_zoom.start_time_ms;
        let progress = (elapsed as f32 / total_duration as f32).clamp(0.0, 1.0);
        
        Some(ZoomState {
            zoom_factor: active_zoom.zoom_factor,
            focus_x: active_zoom.focus_x,
            focus_y: active_zoom.focus_y,
            progress,
            reason: active_zoom.reason.clone(),
        })
    }
    
    /// Reset zoom calculator
    pub fn reset(&mut self) -> Result<()> {
        self.active_zooms.clear();
        self.last_recommendation_time = 0;
        self.focus_smoother.reset();
        self.zoom_stats = ZoomStats::default();
        Ok(())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: &CursorEngineConfig) -> Result<()> {
        self.config = config.clone();
        self.zoom_rules = Self::create_zoom_rules(config)?;
        Ok(())
    }
    
    /// Get zoom statistics
    pub fn get_stats(&self) -> &ZoomStats {
        &self.zoom_stats
    }
    
    // Private implementation methods
    
    fn create_zoom_rules(config: &CursorEngineConfig) -> Result<Vec<ZoomRule>> {
        let mut rules = Vec::new();
        
        // Rule 1: Precise work gets priority zoom
        rules.push(ZoomRule {
            name: "Precise Work Priority".to_string(),
            priority: 100,
            conditions: ZoomConditions {
                requires_precise_work: true,
                requires_hovering: false,
                requires_clicking: false,
                requires_reading: false,
                requires_navigating: false,
                max_velocity: Some(config.precise_velocity_threshold),
                min_stability_score: Some(0.7),
                min_dwell_duration_ms: Some(800),
                ..Default::default()
            },
            zoom_action: ZoomAction {
                zoom_factor: 2.5,
                duration_ms: 4000,
                easing: ZoomEasing::EaseInOut,
                reason: ZoomReason::PreciseWork,
                confidence: 0.9,
                focus_strategy: FocusStrategy::PredictedCursor,
                can_interrupt_existing: true,
            },
        });
        
        // Rule 2: Clicking activity gets moderate zoom
        rules.push(ZoomRule {
            name: "Click Activity Focus".to_string(),
            priority: 90,
            conditions: ZoomConditions {
                requires_clicking: true,
                max_velocity: Some(200.0),
                min_distance_from_edge: Some(50.0), // Away from screen edges
                ..Default::default()
            },
            zoom_action: ZoomAction {
                zoom_factor: 1.8,
                duration_ms: 2000,
                easing: ZoomEasing::EaseIn,
                reason: ZoomReason::ClickActivity,
                confidence: 0.8,
                focus_strategy: FocusStrategy::Cursor,
                can_interrupt_existing: false,
            },
        });
        
        // Rule 3: Stable hovering gets gentle zoom
        rules.push(ZoomRule {
            name: "Stable Hover Focus".to_string(),
            priority: 80,
            conditions: ZoomConditions {
                requires_hovering: true,
                max_velocity: Some(config.hover_velocity_threshold),
                min_stability_score: Some(0.8),
                min_dwell_duration_ms: Some(config.hover_min_duration_ms),
                ..Default::default()
            },
            zoom_action: ZoomAction {
                zoom_factor: 1.5,
                duration_ms: 3000,
                easing: ZoomEasing::EaseOut,
                reason: ZoomReason::HoverFocus,
                confidence: 0.75,
                focus_strategy: FocusStrategy::ActivityCenter,
                can_interrupt_existing: false,
            },
        });
        
        // Rule 4: Reading pattern gets text-optimized zoom
        rules.push(ZoomRule {
            name: "Reading Enhancement".to_string(),
            priority: 70,
            conditions: ZoomConditions {
                requires_reading: true,
                min_velocity: Some(50.0),
                max_velocity: Some(250.0),
                ..Default::default()
            },
            zoom_action: ZoomAction {
                zoom_factor: 1.4,
                duration_ms: 5000, // Longer for reading
                easing: ZoomEasing::EaseInOut,
                reason: ZoomReason::ReadingText,
                confidence: 0.7,
                focus_strategy: FocusStrategy::PatternOptimal,
                can_interrupt_existing: false,
            },
        });
        
        // Rule 5: End of navigation gets focus zoom
        rules.push(ZoomRule {
            name: "Navigation End Focus".to_string(),
            priority: 60,
            conditions: ZoomConditions {
                requires_navigating: false, // Just finished navigating
                max_velocity: Some(100.0), // Slowed down
                min_stability_score: Some(0.6),
                ..Default::default()
            },
            zoom_action: ZoomAction {
                zoom_factor: 1.6,
                duration_ms: 2500,
                easing: ZoomEasing::EaseOut,
                reason: ZoomReason::NavigationEnd,
                confidence: 0.65,
                focus_strategy: FocusStrategy::Cursor,
                can_interrupt_existing: false,
            },
        });
        
        // Sort rules by priority (highest first)
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        Ok(rules)
    }
    
    fn evaluate_rule(&self, rule: &ZoomRule, event: &EnhancedCursorEvent) -> Result<Option<f32>> {
        let conditions = &rule.conditions;
        let mut score = 1.0f32;
        let mut requirements_met = 0;
        let mut total_requirements = 0;
        
        // Check pattern requirements
        if conditions.requires_hovering {
            total_requirements += 1;
            if event.is_hovering {
                requirements_met += 1;
                score *= 1.2; // Bonus for matching
            } else {
                return Ok(None); // Hard requirement not met
            }
        }
        
        if conditions.requires_clicking {
            total_requirements += 1;
            if event.is_clicking {
                requirements_met += 1;
                score *= 1.2;
            } else {
                return Ok(None);
            }
        }
        
        if conditions.requires_precise_work {
            total_requirements += 1;
            if event.is_precise_work {
                requirements_met += 1;
                score *= 1.3; // Higher bonus for precise work
            } else {
                return Ok(None);
            }
        }
        
        if conditions.requires_reading {
            total_requirements += 1;
            if event.is_reading {
                requirements_met += 1;
                score *= 1.1;
            } else {
                return Ok(None);
            }
        }
        
        if conditions.requires_navigating {
            total_requirements += 1;
            if event.is_navigating {
                requirements_met += 1;
                score *= 1.1;
            } else {
                return Ok(None);
            }
        }
        
        // Check motion requirements (soft constraints - reduce score if not met)
        if let Some(max_vel) = conditions.max_velocity {
            if event.velocity > max_vel {
                score *= 0.5; // Penalty for exceeding max velocity
            }
        }
        
        if let Some(min_vel) = conditions.min_velocity {
            if event.velocity < min_vel {
                score *= 0.7; // Penalty for too slow
            }
        }
        
        if let Some(max_acc) = conditions.max_acceleration {
            if event.acceleration.abs() > max_acc {
                score *= 0.8; // Penalty for high acceleration
            }
        }
        
        if let Some(min_stability) = conditions.min_stability_score {
            if event.stability_score < min_stability {
                score *= 0.6; // Penalty for low stability
            }
        }
        
        if let Some(min_dwell) = conditions.min_dwell_duration_ms {
            if event.dwell_duration_ms < min_dwell {
                score *= 0.4; // Strong penalty for insufficient dwell time
            }
        }
        
        // Check distance from screen edges
        if let Some(min_edge_dist) = conditions.min_distance_from_edge {
            let edge_distance = self.calculate_edge_distance(event.x, event.y);
            if edge_distance < min_edge_dist {
                score *= 0.3; // Strong penalty for being too close to edge
            }
        }
        
        // Apply confidence based on how well requirements are met
        if total_requirements > 0 {
            let requirement_ratio = requirements_met as f32 / total_requirements as f32;
            score *= requirement_ratio;
        }
        
        // Clamp score and apply rule's base confidence
        score = (score * rule.zoom_action.confidence).clamp(0.0, 1.0);
        
        Ok(Some(score))
    }
    
    fn create_recommendation(
        &mut self,
        rule: &ZoomRule,
        event: &EnhancedCursorEvent,
        confidence: f32,
    ) -> Result<ZoomRecommendation> {
        let action = &rule.zoom_action;
        
        // Calculate focus point based on strategy
        let (focus_x, focus_y) = self.calculate_focus_point(&action.focus_strategy, event)?;
        
        // Apply focus smoothing
        let (smoothed_x, smoothed_y) = self.focus_smoother.smooth_focus(focus_x, focus_y, event);
        
        Ok(ZoomRecommendation {
            start_time_ms: event.timestamp_ms + 50, // Small delay for natural feel
            end_time_ms: event.timestamp_ms + action.duration_ms,
            focus_x: smoothed_x,
            focus_y: smoothed_y,
            zoom_factor: action.zoom_factor.clamp(self.config.min_zoom_factor, self.config.max_zoom_factor),
            confidence,
            reason: action.reason.clone(),
            easing: action.easing.clone(),
        })
    }
    
    fn calculate_focus_point(
        &self,
        strategy: &FocusStrategy,
        event: &EnhancedCursorEvent,
    ) -> Result<(f32, f32)> {
        match strategy {
            FocusStrategy::Cursor => {
                Ok((
                    (event.x / self.config.screen_width as f64) as f32,
                    (event.y / self.config.screen_height as f64) as f32,
                ))
            }
            FocusStrategy::PredictedCursor => {
                // Predict cursor position based on velocity and direction
                let prediction_time_ms = 200.0; // Predict 200ms ahead
                let prediction_distance = event.velocity * (prediction_time_ms / 1000.0);
                
                let predicted_x = event.x + prediction_distance * event.direction.cos();
                let predicted_y = event.y + prediction_distance * event.direction.sin();
                
                Ok((
                    (predicted_x / self.config.screen_width as f64).clamp(0.0, 1.0) as f32,
                    (predicted_y / self.config.screen_height as f64).clamp(0.0, 1.0) as f32,
                ))
            }
            FocusStrategy::ActivityCenter => {
                // Use current position as activity center for now
                // In a more advanced implementation, this would calculate the center
                // of recent high-activity areas
                Ok((
                    (event.x / self.config.screen_width as f64) as f32,
                    (event.y / self.config.screen_height as f64) as f32,
                ))
            }
            FocusStrategy::PatternOptimal => {
                // Optimize focus point based on detected pattern
                // For reading, focus slightly ahead in reading direction
                if event.is_reading {
                    let reading_offset = 50.0; // pixels ahead
                    let focus_x = ((event.x + reading_offset) / self.config.screen_width as f64) as f32;
                    Ok((focus_x.clamp(0.0, 1.0), (event.y / self.config.screen_height as f64) as f32))
                } else {
                    // Default to cursor position
                    Ok((
                        (event.x / self.config.screen_width as f64) as f32,
                        (event.y / self.config.screen_height as f64) as f32,
                    ))
                }
            }
        }
    }
    
    fn calculate_edge_distance(&self, x: f64, y: f64) -> f64 {
        let dist_to_left = x;
        let dist_to_right = self.config.screen_width as f64 - x;
        let dist_to_top = y;
        let dist_to_bottom = self.config.screen_height as f64 - y;
        
        dist_to_left.min(dist_to_right).min(dist_to_top).min(dist_to_bottom)
    }
    
    fn can_recommend_zoom(&self, current_time_ms: u64) -> bool {
        current_time_ms - self.last_recommendation_time >= self.config.zoom_cooldown_ms
    }
    
    fn cleanup_expired_zooms(&mut self, current_time_ms: u64) {
        self.active_zooms.retain(|zoom| zoom.end_time_ms > current_time_ms);
    }
    
    fn has_conflicting_zoom(&self, recommendation: &ZoomRecommendation) -> bool {
        // Check if there's an overlapping zoom that would conflict
        self.active_zooms.iter().any(|zoom| {
            // Check time overlap
            let time_overlap = !(recommendation.end_time_ms < zoom.start_time_ms || 
                               recommendation.start_time_ms > zoom.end_time_ms);
            
            // Check spatial overlap (focus points close together)
            if time_overlap {
                let focus_distance = ((recommendation.focus_x - zoom.focus_x).powi(2) + 
                                     (recommendation.focus_y - zoom.focus_y).powi(2)).sqrt();
                focus_distance < 0.3 // 30% of screen distance
            } else {
                false
            }
        })
    }
    
    fn clear_lower_priority_zooms(&mut self, priority: u32) {
        self.active_zooms.retain(|zoom| zoom.priority >= priority);
    }
    
    fn create_active_zoom(&mut self, recommendation: &ZoomRecommendation, priority: u32) {
        let zoom_id = format!("zoom_{}", recommendation.start_time_ms);
        
        self.active_zooms.push_back(ActiveZoom {
            id: zoom_id,
            start_time_ms: recommendation.start_time_ms,
            end_time_ms: recommendation.end_time_ms,
            focus_x: recommendation.focus_x,
            focus_y: recommendation.focus_y,
            zoom_factor: recommendation.zoom_factor,
            reason: recommendation.reason.clone(),
            priority,
        });
    }
    
    fn update_zoom_stats(&mut self, recommendation: &ZoomRecommendation) {
        self.zoom_stats.successful_zooms += 1;
        
        // Update reason statistics
        let reason_key = format!("{:?}", recommendation.reason);
        *self.zoom_stats.zoom_by_reason.entry(reason_key).or_insert(0) += 1;
        
        // Update averages
        let total_zooms = self.zoom_stats.successful_zooms as f32;
        self.zoom_stats.avg_zoom_factor = 
            (self.zoom_stats.avg_zoom_factor * (total_zooms - 1.0) + recommendation.zoom_factor) / total_zooms;
        
        let duration = recommendation.end_time_ms - recommendation.start_time_ms;
        self.zoom_stats.avg_zoom_duration_ms = 
            ((self.zoom_stats.avg_zoom_duration_ms as f32 * (total_zooms - 1.0)) + duration as f32) as u64 / total_zooms as u64;
    }
}

impl Default for ZoomConditions {
    fn default() -> Self {
        Self {
            requires_hovering: false,
            requires_clicking: false,
            requires_precise_work: false,
            requires_reading: false,
            requires_navigating: false,
            max_velocity: None,
            min_velocity: None,
            max_acceleration: None,
            min_stability_score: None,
            min_dwell_duration_ms: None,
            min_distance_from_edge: None,
        }
    }
}

impl FocusSmoother {
    fn new(smoothing_factor: f32, prediction_weight: f32) -> Self {
        Self {
            last_focus: None,
            smoothing_factor: smoothing_factor.clamp(0.0, 1.0),
            prediction_weight: prediction_weight.clamp(0.0, 1.0),
        }
    }
    
    fn smooth_focus(&mut self, focus_x: f32, focus_y: f32, _event: &EnhancedCursorEvent) -> (f32, f32) {
        if let Some((last_x, last_y)) = self.last_focus {
            // Apply exponential smoothing
            let smoothed_x = self.smoothing_factor * focus_x + (1.0 - self.smoothing_factor) * last_x;
            let smoothed_y = self.smoothing_factor * focus_y + (1.0 - self.smoothing_factor) * last_y;
            
            self.last_focus = Some((smoothed_x, smoothed_y));
            (smoothed_x, smoothed_y)
        } else {
            self.last_focus = Some((focus_x, focus_y));
            (focus_x, focus_y)
        }
    }
    
    fn reset(&mut self) {
        self.last_focus = None;
    }
}

/// Current zoom state for rendering
#[derive(Debug, Clone)]
pub struct ZoomState {
    pub zoom_factor: f32,
    pub focus_x: f32,
    pub focus_y: f32,
    pub progress: f32, // 0.0 - 1.0
    pub reason: ZoomReason,
}