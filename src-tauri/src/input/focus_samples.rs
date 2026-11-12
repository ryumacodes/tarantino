use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusTrigger { Key, Pointer, Modifiers, Accessibility, Application, Activation }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusState { Present, Absent, #[default] Unavailable }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FocusSample {
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_state: Option<FocusState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_pid: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_bounds: Option<[f64; 4]>,
    pub caret: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret_bounds: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_caret: Option<[f64; 2]>,
    pub trigger_timestamp: u64,
    pub trigger: FocusTrigger,
}

pub fn normalized(samples: &[FocusSample], start: u64) -> Vec<FocusSample> {
    samples.iter().filter(|sample| sample.timestamp >= start && sample.trigger_timestamp >= start).map(|sample| FocusSample {
        timestamp: sample.timestamp - start,
        focus_state: sample.focus_state,
        app_pid: sample.app_pid,
        focus_role: sample.focus_role.clone(),
        focus_bounds: sample.focus_bounds,
        caret: sample.caret,
        caret_bounds: sample.caret_bounds,
        estimated_caret: sample.estimated_caret,
        trigger_timestamp: sample.trigger_timestamp - start,
        trigger: sample.trigger,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_survive_normalization_without_changing_measured_carets() {
        let old = r#"{"timestamp":200,"caret":null,"trigger_timestamp":100,"trigger":"key"}"#;
        let mut sample: FocusSample = serde_json::from_str(old).unwrap();
        assert_eq!(sample.focus_state, None);
        assert_eq!(sample.app_pid, None);
        assert_eq!(sample.focus_role, None);
        assert_eq!(sample.focus_bounds, None);
        assert_eq!(sample.estimated_caret, None);
        assert_eq!(sample.caret_bounds, None);
        sample.estimated_caret = Some([10.0, 20.0]);
        sample.caret_bounds = Some([9.0, 12.0, 2.0, 16.0]);
        let result = normalized(std::slice::from_ref(&sample), 100);
        assert_eq!(result[0].timestamp, 100);
        assert_eq!(result[0].caret, None);
        assert_eq!(result[0].caret_bounds, sample.caret_bounds);
        assert_eq!(result[0].estimated_caret, Some([10.0, 20.0]));
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(serde_json::from_str::<Vec<FocusSample>>(&json).unwrap(), result);
    }

    #[test]
    fn focus_state_is_independent_of_caret_availability_and_survives_normalization() {
        for state in [FocusState::Present, FocusState::Absent, FocusState::Unavailable] {
            let sample = FocusSample {
                timestamp: 200, trigger_timestamp: 100, trigger: FocusTrigger::Accessibility,
                app_pid: (state == FocusState::Present).then_some(42),
                focus_role: (state == FocusState::Present).then(|| "AXTextField".to_owned()),
                focus_bounds: (state == FocusState::Present).then_some([-200.0, 100.0, 400.0, 300.0]),
                focus_state: Some(state), caret: None, caret_bounds: None, estimated_caret: None,
            };
            let samples = normalized(&[sample], 100);
            assert_eq!(samples[0].focus_state, Some(state));
            assert_eq!(samples[0].app_pid, (state == FocusState::Present).then_some(42));
            assert_eq!(samples[0].focus_role.as_deref(), (state == FocusState::Present).then_some("AXTextField"));
            assert_eq!(samples[0].focus_bounds, (state == FocusState::Present).then_some([-200.0, 100.0, 400.0, 300.0]));
            assert_eq!(samples[0].caret, None);
            let saved = serde_json::to_vec(&samples).unwrap();
            assert_eq!(serde_json::from_slice::<Vec<FocusSample>>(&saved).unwrap(), samples);
        }
    }

    #[test]
    fn focus_timestamps_remain_independent_and_unavailable_samples_survive() {
        let samples = vec![
            FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 150, trigger_timestamp: 90, trigger: FocusTrigger::Key, caret: Some([5.0, 6.0]) },
            FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 99, trigger_timestamp: 98, trigger: FocusTrigger::Key, caret: Some([1.0, 2.0]) },
            FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 100, trigger_timestamp: 100, trigger: FocusTrigger::Pointer, caret: Some([3.0, 4.0]) },
            FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 205, trigger_timestamp: 105, trigger: FocusTrigger::Key, caret: None },
        ];
        let normalized = normalized(&samples, 100);
        assert_eq!(normalized, vec![FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 0, trigger_timestamp: 0, trigger: FocusTrigger::Pointer, caret: Some([3.0, 4.0]) },
            FocusSample { app_pid: None, focus_role: None, focus_bounds: None, focus_state: None, caret_bounds: None, estimated_caret: None, timestamp: 105, trigger_timestamp: 5, trigger: FocusTrigger::Key, caret: None }]);
        let json = serde_json::to_string(&normalized).unwrap();
        assert_eq!(serde_json::from_str::<Vec<FocusSample>>(&json).unwrap(), normalized);
    }
}
