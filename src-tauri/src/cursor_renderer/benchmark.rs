//! Opt-in CPU measurement; no display, permissions, or recording content required.
use super::*;
use crate::video_processing::{CursorSettings, export_range::ExportRange};
use std::time::Instant;

#[test]
#[ignore = "synthetic ten-minute cursor memory/timing profile"]
fn ten_minute_cursor_profile() {
    let events: Vec<_> = (0..36_000)
        .map(|i| CursorEvent { is_release: false,
            timestamp_ms: i * 1000 / 60,
            x: (i as f64 / 200.0).sin() * 0.4 + 0.5,
            y: (i as f64 / 300.0).cos() * 0.4 + 0.5,
            is_click: i % 120 == 0,
        })
        .collect();
    let spring = SpringConfig {
        tension: 470.0,
        friction: 70.0,
        mass: 3.0,
    };
    for trails in [false, true] {
        let settings: CursorSettings =
            serde_json::from_value(serde_json::json!({"trail_enabled":trails,"trail_length":15}))
                .unwrap();
        let start = Instant::now();
        let states = simulate_cursor_positions_for_range(
            &events,
            &spring,
            120.0,
            ExportRange {
                start_ms: 0,
                end_ms: 600_000,
            },
            &None,
            &settings,
        );
        let elapsed = start.elapsed();
        let mut checksum = 0xcbf29ce484222325_u64;
        for state in &states {
            for value in [
                state.x,
                state.y,
                state.opacity,
                state.rotation,
                state.ripple_progress,
                state.trail_count,
            ]
            .into_iter()
            .chain(
                state
                    .trail_points()
                    .iter()
                    .take(state.trail_count as usize)
                    .flat_map(|p| [p.x, p.y, p.alpha, p.size]),
            ) {
                checksum = (checksum ^ value.to_bits() as u64).wrapping_mul(0x100000001b3);
            }
        }
        println!("CURSOR_CHECKSUM trails={trails} {checksum:016x}");
        assert_eq!(
            checksum,
            if trails {
                0xaa6efac2db0c4bae
            } else {
                0xa4159bd820983646
            }
        );
        let mut allocations = std::collections::HashSet::new();
        let payload_bytes: usize = states
            .iter()
            .map(|s| {
                if s.trail_points().is_empty() || !allocations.insert(s.trail_points().as_ptr()) {
                    0
                } else {
                    std::mem::size_of_val(s.trail_points()) + 2 * std::mem::size_of::<usize>()
                }
            })
            .sum();
        println!("CURSOR_PAYLOAD trails={trails} allocated_bytes={payload_bytes}");
        assert_eq!(states.len(), 72_000);
        assert!(states.iter().all(|s| s.x.is_finite() && s.y.is_finite()));
        println!(
            "CURSOR_PROFILE trails={trails} frames={} inline_bytes={} elapsed_ms={:.3}",
            states.len(),
            states.capacity() * std::mem::size_of::<CursorFrameState>(),
            elapsed.as_secs_f64() * 1000.0
        );
    }
}
