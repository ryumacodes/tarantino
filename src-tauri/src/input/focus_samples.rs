use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FocusSample {
    pub timestamp: u64,
    pub caret: Option<[f64; 2]>,
}

pub fn normalized(samples: &[FocusSample], start: u64) -> Vec<FocusSample> {
    samples.iter().filter(|sample| sample.timestamp >= start).map(|sample| FocusSample {
        timestamp: sample.timestamp - start,
        caret: sample.caret,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_timestamps_remain_independent_and_unavailable_samples_survive() {
        let samples = vec![
            FocusSample { timestamp: 99, caret: Some([1.0, 2.0]) },
            FocusSample { timestamp: 100, caret: Some([3.0, 4.0]) },
            FocusSample { timestamp: 205, caret: None },
        ];
        let normalized = normalized(&samples, 100);
        assert_eq!(normalized, vec![FocusSample { timestamp: 0, caret: Some([3.0, 4.0]) },
            FocusSample { timestamp: 105, caret: None }]);
        let json = serde_json::to_string(&normalized).unwrap();
        assert_eq!(serde_json::from_str::<Vec<FocusSample>>(&json).unwrap(), normalized);
    }
}
