use super::Sample;

pub fn decode_spans(samples: &[Sample]) -> Vec<[f64; 2]> {
    let mut movement: Vec<[f64; 2]> = Vec::new();
    for sample in samples.iter().filter(|sample| sample.free) {
        if let Some(last) = movement.last_mut()
            && sample.time_ms - last[1] <= 1000.0
        {
            last[1] = sample.time_ms + 150.0;
        } else {
            movement.push([sample.time_ms, sample.time_ms + 150.0]);
        }
    }
    let mut decode: Vec<[f64; 2]> = Vec::new();
    for [start, end] in movement {
        // Include settled history and decoder warmup before activity resumes.
        let start = (start - 1270.0).max(0.0);
        if let Some(last) = decode.last_mut()
            && start <= last[1]
        {
            last[1] = end;
        } else {
            decode.push([start, end]);
        }
    }
    decode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_idle_gaps_and_merges_overlapping_warmup() {
        let samples = [0.0, 1000.0, 2250.0, 8000.0].map(|time_ms| Sample {
            time_ms, x: 0.0, y: 0.0, free: true,
        });
        assert_eq!(decode_spans(&samples), vec![[0.0, 2400.0], [6730.0, 8150.0]]);
        let dragged = samples.map(|sample| Sample { free: false, ..sample });
        assert!(decode_spans(&dragged).is_empty());
    }
}
