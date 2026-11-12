use crate::{
    event_capture::EnhancedMouseEvent,
    input::{
        KeyEvent, MouseEventType,
        focus_samples::{FocusSample, FocusTrigger},
    },
};
use std::collections::{HashMap, HashSet};

pub(super) fn associated<'a>(
    keys: &[KeyEvent],
    mouse: &[EnhancedMouseEvent],
    samples: &'a [FocusSample],
    duration: u64,
) -> HashMap<u64, &'a FocusSample> {
    if samples.is_empty() { return HashMap::new(); }
    let typing: HashSet<_> = keys
        .iter()
        .filter(|key| key.is_typing && !key.is_release)
        .map(|key| key.timestamp)
        .collect();
    let mut barriers: Vec<_> = keys
        .iter()
        .filter(|key| !key.is_release && !key.is_modifier && !key.is_typing)
        .map(|key| key.timestamp)
        .chain(
            mouse
                .iter()
                .filter(|event| matches!(event.base.event_type, MouseEventType::ButtonPress { .. }))
                .map(|event| event.base.timestamp),
        )
        .chain(
            samples
                .iter()
                .filter(|sample| sample.trigger == FocusTrigger::Activation)
                .map(|sample| sample.trigger_timestamp),
        )
        .collect();
    barriers.sort_unstable();
    samples
        .iter()
        .filter(|sample| {
            sample.trigger == FocusTrigger::Key
                && typing.contains(&sample.trigger_timestamp)
                && sample.timestamp >= sample.trigger_timestamp
                && sample.timestamp <= duration
                && sample
                    .caret
                    .is_some_and(|point| point.iter().all(|value| value.is_finite()))
                && barriers
                    .get(barriers.partition_point(|time| *time < sample.trigger_timestamp))
                    .is_none_or(|time| *time > sample.timestamp)
        })
        .map(|sample| (sample.trigger_timestamp, sample))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{MouseButton, MouseEvent};
    fn key(time: u64, typing: bool) -> KeyEvent {
        let mut key: KeyEvent = serde_json::from_value(
            serde_json::json!({"timestamp":time,"is_modifier":false,"is_typing":false}),
        )
        .unwrap();
        key.is_release = false;
        key.is_typing = typing;
        key
    }
    fn sample(trigger: u64, observed: u64) -> FocusSample {
        FocusSample {
            focus_state: None,
            app_pid: None,
            focus_role: None,
            focus_bounds: None,
            caret_bounds: None,
            estimated_caret: None,
            timestamp: observed,
            trigger_timestamp: trigger,
            trigger: FocusTrigger::Key,
            caret: Some([20.0, 30.0]),
        }
    }
    #[test]
    fn exact_key_join_preserves_observation_time_and_rejects_other_triggers() {
        let keys = [key(100, true), key(200, true)];
        let mut samples = vec![sample(100, 215), sample(101, 220), sample(200, 300)];
        samples[2].trigger = FocusTrigger::Pointer;
        let result = associated(&keys, &[], &samples, 1000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[&100].timestamp, 215);
    }
    #[test]
    fn estimated_focus_is_not_a_measured_typing_target() {
        let mut focus = sample(100, 200);
        focus.estimated_caret = focus.caret.take();
        assert!(associated(&[key(100, true)], &[], &[focus], 1000).is_empty());
    }

    #[test]
    fn application_switch_prevents_a_delayed_caret_from_retargeting_earlier_typing() {
        let mut activation = sample(150, 250);
        activation.trigger = FocusTrigger::Activation;
        activation.caret = None;
        let samples = [sample(100, 200), activation, sample(300, 400)];
        let result = associated(&[key(100, true), key(300, true)], &[], &samples, 1000);
        assert!(!result.contains_key(&100));
        assert_eq!(result[&300].timestamp, 400);
    }

    #[test]
    fn intervening_navigation_or_click_cannot_retarget_typing() {
        let samples = [sample(100, 220)];
        assert!(associated(&[key(100, true), key(200, false)], &[], &samples, 1000).is_empty());
        let mouse = [EnhancedMouseEvent {
            base: MouseEvent {
                timestamp: 200,
                x: 20.0,
                y: 30.0,
                event_type: MouseEventType::ButtonPress {
                    button: MouseButton::Left,
                },
                display_id: None,
            },
            window_id: None,
            app_name: None,
            is_double_click: false,
            cluster_id: None,
        }];
        assert!(associated(&[key(100, true)], &mouse, &samples, 1000).is_empty());
        assert!(associated(&[key(100, true)], &[], &samples, 200).is_empty());
    }
}
