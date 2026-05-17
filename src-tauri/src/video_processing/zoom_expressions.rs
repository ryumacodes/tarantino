//! FFmpeg zoom and pan expression builders
//!
//! Builds complex FFmpeg filter expressions for smooth zoom transitions.
//! Uses smoothstep easing to approximate critically-damped spring physics.

use super::types::ZoomBlock;

/// Smoothstep easing helper for FFmpeg expressions.
/// Takes a linear 0-1 progress expression and returns smoothstep(t) = t*t*(3-2*t)
fn smoothstep_expr(linear_expr: &str) -> String {
    // Store the clamped linear value, then apply smoothstep
    format!(
        "st(0\\,clip({linear_expr}\\,0\\,1))*ld(0)*ld(0)*(3-2*ld(0))",
    )
}

/// Build the eased progress expression for a zoom block.
/// Returns a value 0-1 representing the zoom progress with smooth in/out.
fn block_progress_expr(start_frame: u64, end_frame: u64, transition_frames: u64) -> String {
    let trans = transition_frames.max(1);

    // Ease-in progress: smoothstep from start to start+transition
    let ease_in = smoothstep_expr(&format!("(on-{})/{}",start_frame, trans));
    // Ease-out progress: smoothstep from end-transition to end
    let ease_out = smoothstep_expr(&format!("({}-on)/{}", end_frame, trans));

    format!(
        "min({}\\,{})*between(on\\,{}\\,{})",
        ease_in, ease_out, start_frame, end_frame
    )
}

/// Build FFmpeg expression for zoom level based on zoom blocks
pub fn build_zoom_expression(zoom_blocks: &[ZoomBlock], fps: f64) -> String {
    if zoom_blocks.is_empty() {
        return "1".to_string();
    }

    // 400ms transition to match preview spring physics
    let transition_frames = (fps * 0.4).max(1.0) as u64;
    let mut block_exprs: Vec<String> = Vec::new();

    for block in zoom_blocks.iter() {
        let start_sec = block.start_time_ms as f64 / 1000.0;
        let end_sec = block.end_time_ms as f64 / 1000.0;
        let zoom = block.zoom_level;
        let start_frame = (start_sec * fps) as u64;
        let end_frame = (end_sec * fps) as u64;

        let progress = block_progress_expr(start_frame, end_frame, transition_frames);
        let block_expr = format!("1+{}*{}", zoom - 1.0, progress);
        block_exprs.push(block_expr);
    }

    if block_exprs.len() == 1 {
        block_exprs[0].clone()
    } else {
        let mut result = "1".to_string();
        for expr in block_exprs.iter().rev() {
            result = format!("max({}\\,{})", expr, result);
        }
        result
    }
}

/// Build FFmpeg expression for horizontal pan based on zoom blocks
/// Includes edge clamping to prevent showing outside video bounds
pub fn build_pan_x_expression(zoom_blocks: &[ZoomBlock], fps: f64) -> String {
    if zoom_blocks.is_empty() {
        return "iw/2-(iw/zoom/2)".to_string();
    }

    let transition_frames = (fps * 0.4).max(1.0) as u64;
    let mut parts: Vec<String> = Vec::new();

    for block in zoom_blocks.iter() {
        let start_sec = block.start_time_ms as f64 / 1000.0;
        let end_sec = block.end_time_ms as f64 / 1000.0;
        let start_frame = (start_sec * fps) as u64;
        let end_frame = (end_sec * fps) as u64;
        let center_x = block.center_x;

        let progress = block_progress_expr(start_frame, end_frame, transition_frames);
        let delta_x = center_x - 0.5;

        parts.push(format!("{}*{}*iw", delta_x, progress));
    }

    // Base pan + offsets, clamped to valid range
    let offset_expr = if parts.len() == 1 {
        format!("iw/2-iw/zoom/2+{}", parts[0])
    } else {
        let sum = parts.join("+");
        format!("iw/2-iw/zoom/2+{}", sum)
    };

    // Clamp: min=0, max=iw-iw/zoom
    format!("clip({}\\,0\\,iw-iw/zoom)", offset_expr)
}

/// Build FFmpeg expression for vertical pan based on zoom blocks
/// Includes edge clamping to prevent showing outside video bounds
pub fn build_pan_y_expression(zoom_blocks: &[ZoomBlock], fps: f64) -> String {
    if zoom_blocks.is_empty() {
        return "ih/2-(ih/zoom/2)".to_string();
    }

    let transition_frames = (fps * 0.4).max(1.0) as u64;
    let mut parts: Vec<String> = Vec::new();

    for block in zoom_blocks.iter() {
        let start_sec = block.start_time_ms as f64 / 1000.0;
        let end_sec = block.end_time_ms as f64 / 1000.0;
        let start_frame = (start_sec * fps) as u64;
        let end_frame = (end_sec * fps) as u64;
        let center_y = block.center_y;

        let progress = block_progress_expr(start_frame, end_frame, transition_frames);
        let delta_y = center_y - 0.5;

        parts.push(format!("{}*{}*ih", delta_y, progress));
    }

    let offset_expr = if parts.len() == 1 {
        format!("ih/2-ih/zoom/2+{}", parts[0])
    } else {
        let sum = parts.join("+");
        format!("ih/2-ih/zoom/2+{}", sum)
    };

    // Clamp: min=0, max=ih-ih/zoom
    format!("clip({}\\,0\\,ih-ih/zoom)", offset_expr)
}
