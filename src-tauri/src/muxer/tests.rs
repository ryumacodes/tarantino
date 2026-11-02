use super::*;
use bytes::Bytes;

// Container timing test: packet bytes are placeholders, not a decoding fixture.
fn frame(timestamp_us: u64) -> EncodedFrame {
    EncodedFrame {
        data: Bytes::from_static(&[0, 0, 0, 1, 0x65, 0x88]),
        timestamp_us, is_keyframe: true, pts: 0, dts: 0,
        sps: Some(Bytes::from_static(&[0x67, 0x42, 0, 0x1e, 0x89, 0x8b, 0x60])),
        pps: Some(Bytes::from_static(&[0x68, 0xce, 0x06, 0xe2])),
    }
}

#[test]
fn persisted_sample_times_do_not_accumulate_millisecond_rounding() {
    for fps in [24_u32, 30, 60, 120] {
        let path = std::env::temp_dir().join(format!("mux-clock-{}.mp4", uuid::Uuid::new_v4()));
        let count = fps * 60;
        let origin = 987_654_321_123_u64;
        let mut mux = Mp4Muxer::new(&path, 128, 128, fps).unwrap();
        for index in 0..count {
            let offset = (index as u64 * 1_000_000 + fps as u64 / 2) / fps as u64;
            mux.write_frame(&frame(origin + offset), 17).unwrap();
        }
        mux.finish().unwrap();
        let file = File::open(&path).unwrap();
        let size = file.metadata().unwrap().len();
        let mut reader = mp4::Mp4Reader::read_header(file, size).unwrap();
        assert_eq!(reader.sample_count(1).unwrap(), count);
        for index in 0..count {
            let sample = reader.read_sample(1, index + 1).unwrap().unwrap();
            assert_eq!(sample.start_time, u64::from(index) * 1000, "fps={fps}, frame={index}");
            assert_eq!(sample.duration, 1000, "fps={fps}, frame={index}");
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn capture_gaps_and_static_tail_keep_their_source_duration() {
    let path = std::env::temp_dir().join(format!("mux-gaps-{}.mp4", uuid::Uuid::new_v4()));
    let mut mux = Mp4Muxer::new(&path, 128, 128, 60).unwrap();
    let offsets = [0, 16_667, 50_000, 1_050_001];
    for offset in offsets { mux.write_frame(&frame(9_000_000 + offset), 17).unwrap(); }
    mux.finish_with_duration(Some(2000)).unwrap();
    let file = File::open(&path).unwrap();
    let mut reader = mp4::Mp4Reader::read_header(file, std::fs::metadata(&path).unwrap().len()).unwrap();
    let samples: Vec<_> = (1..=4).map(|i| reader.read_sample(1, i).unwrap().unwrap()).collect();
    assert_eq!(samples.iter().map(|s| s.start_time).collect::<Vec<_>>(), [0,1000,3000,63000]);
    assert_eq!(samples.iter().map(|s| s.duration).collect::<Vec<_>>(), [1000,2000,60000,57000]);
    std::fs::remove_file(path).unwrap();
}
