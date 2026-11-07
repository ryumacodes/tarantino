use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusTrigger { Key, Pointer, Modifiers, Accessibility, Application }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FocusSample {
    pub timestamp: u64,
    pub caret: Option<[f64; 2]>,
    pub trigger_timestamp: u64,
    pub trigger: FocusTrigger,
}

pub fn normalized(samples: &[FocusSample], start: u64) -> Vec<FocusSample> {
    samples.iter().filter(|sample| sample.timestamp >= start && sample.trigger_timestamp >= start).map(|sample| FocusSample {
        timestamp: sample.timestamp - start,
        caret: sample.caret,
        trigger_timestamp: sample.trigger_timestamp - start,
        trigger: sample.trigger,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_timestamps_remain_independent_and_unavailable_samples_survive() {
        let samples = vec![
            FocusSample { timestamp: 150, trigger_timestamp: 90, trigger: FocusTrigger::Key, caret: Some([5.0, 6.0]) },
            FocusSample { timestamp: 99, trigger_timestamp: 98, trigger: FocusTrigger::Key, caret: Some([1.0, 2.0]) },
            FocusSample { timestamp: 100, trigger_timestamp: 100, trigger: FocusTrigger::Pointer, caret: Some([3.0, 4.0]) },
            FocusSample { timestamp: 205, trigger_timestamp: 105, trigger: FocusTrigger::Key, caret: None },
        ];
        let normalized = normalized(&samples, 100);
        assert_eq!(normalized, vec![FocusSample { timestamp: 0, trigger_timestamp: 0, trigger: FocusTrigger::Pointer, caret: Some([3.0, 4.0]) },
            FocusSample { timestamp: 105, trigger_timestamp: 5, trigger: FocusTrigger::Key, caret: None }]);
        let json = serde_json::to_string(&normalized).unwrap();
        assert_eq!(serde_json::from_str::<Vec<FocusSample>>(&json).unwrap(), normalized);
    }
}
