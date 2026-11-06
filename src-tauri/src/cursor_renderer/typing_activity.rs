//! Recorded keyboard activity used by both cursor render paths.
#[derive(Default)]
pub(crate) struct TypingActivity {
    keys: Vec<u64>,
    all_keys: Vec<u64>,
    pointer: Vec<u64>,
}
impl TypingActivity {
    pub(crate) fn load(path: &std::path::Path) -> Self {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .map(|value| Self::from_value(&value)).unwrap_or_default()
    }
    fn from_value(value: &serde_json::Value) -> Self {
        let all = value["key_events"].as_array().into_iter().flatten();
        let mut all_keys: Vec<_> = all.clone().filter_map(|key| key["timestamp"].as_u64()).collect();
        let mut keys: Vec<_> = all
            .filter(|key| key["is_release"].as_bool() != Some(true))
            .filter_map(|key| key["timestamp"].as_u64()).collect();
        let mut pointer: Vec<_> = value["mouse_events"].as_array().into_iter().flatten()
            .filter_map(|event| event.get("base").unwrap_or(event)["timestamp"].as_u64()).collect();
        keys.sort_unstable();
        all_keys.sort_unstable();
        pointer.sort_unstable();
        Self { keys, all_keys, pointer }
    }
    pub(crate) fn active(&self, time: f64) -> bool {
        if !time.is_finite() || time < 0.0 { return false; }
        let end = self.keys.partition_point(|&key| key as f64 <= time);
        let start = self.keys.partition_point(|&key| (key as f64) < time - 1800.0);
        if end - start < 3 { return false; }
        let latest_key = self.all_keys[self.all_keys.partition_point(|&key| key as f64 <= time) - 1];
        let pointer_end = self.pointer.partition_point(|&point| point as f64 <= time);
        pointer_end == 0 || self.pointer[pointer_end - 1] <= latest_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_renderer::{CursorEvent, SpringConfig, simulate_cursor_frames_with_activity,
        simulate_cursor_range_with_activity};
    use crate::video_processing::export_range::ExportRange;
    fn recording() -> serde_json::Value {
        serde_json::json!({"display_width":100,"display_height":100,
            "mouse_events":[{"timestamp":0,"x":20,"y":20}, {"timestamp":1000,"x":30,"y":20}],
            "key_events":[{"timestamp":100,"is_typing":true}, {"timestamp":200,"is_typing":true},
                {"timestamp":300,"is_typing":true}]})
    }
    #[test]
    fn activity_requires_three_recent_keys_and_yields_to_later_pointer_activity() {
        let activity = TypingActivity::from_value(&recording());
        assert!(!activity.active(299.9));
        assert!(activity.active(300.0));
        assert!(activity.active(999.9));
        assert!(!activity.active(1000.0));
        let mut input = recording();
        input["mouse_events"] = serde_json::json!([]);
        let activity = TypingActivity::from_value(&input);
        assert!(activity.active(1900.0));
        assert!(!activity.active(1900.1));
        assert!(!activity.active(f64::NAN));
        input["key_events"][1]["is_typing"] = false.into();
        assert!(TypingActivity::from_value(&input).active(500.0));
        assert!(!TypingActivity::default().active(500.0));
    }

    #[test]
    fn shortcut_chords_and_repeats_count_without_requiring_character_data() {
        let input = serde_json::json!({"key_events":[
            {"timestamp":100,"is_modifier":true,"is_typing":false},
            {"timestamp":100,"is_modifier":true,"is_typing":false},
            {"timestamp":200,"is_modifier":false,"is_typing":false}],
            "mouse_events":[{"base":{"timestamp":200}},{"base":{"timestamp":201}}]});
        let activity = TypingActivity::from_value(&input);
        assert!(!activity.active(199.0));
        assert!(activity.active(200.0)); // Equal-time pointer does not take precedence.
        assert!(!activity.active(201.0));
        let repeats = TypingActivity::from_value(&serde_json::json!({"key_events":[
            {"timestamp":100}, {"timestamp":200}, {"timestamp":300}]}));
        assert!(repeats.active(500.0));
        assert!(!repeats.active(2000.0));
    }

    #[test]
    fn releases_restore_keyboard_precedence_but_never_count_as_presses() {
        let activity = TypingActivity::from_value(&serde_json::json!({"key_events":[
            {"timestamp":100}, {"timestamp":200}, {"timestamp":300},
            {"timestamp":500,"is_release":true}],
            "mouse_events":[{"timestamp":400},{"timestamp":600}]}));
        assert!(activity.active(300.0));
        assert!(!activity.active(400.0));
        assert!(activity.active(500.0));
        assert!(!activity.active(600.0));
        assert!(!activity.active(2000.0));
        let activity = TypingActivity::from_value(&serde_json::json!({"key_events":[
            {"timestamp":100}, {"timestamp":200,"is_release":true},
            {"timestamp":300,"is_release":true}]}));
        assert!(!activity.active(300.0));
    }
    #[tokio::test]
    async fn native_preview_and_trimmed_export_share_typing_fade_at_all_frame_rates() {
        let input = recording();
        let activity = TypingActivity::from_value(&input);
        let path = std::env::temp_dir().join(format!("typing-cursor-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path, input.to_string()).unwrap();
        let events = [CursorEvent{ is_release: false,timestamp_ms:0,x:0.2,y:0.2,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:1000,x:0.3,y:0.2,is_click:false}];
        let spring = SpringConfig { tension:170.0, friction:30.0,mass:1.0 };
        let settings = serde_json::from_value(serde_json::json!({})).unwrap();
        let typing = |time| activity.active(time);
        for fps in [24,30,60,120] {
            let response = crate::commands::video::compute_cursor_trajectory(
                path.to_string_lossy().into_owned(),2500,fps,100,100,1.0,false,2000,false,0,
                false,None,None,None,None,None,None,None,None, None,None,None,Some(false), None).await.unwrap();
            let preview: Vec<[f32;10]> = serde_json::from_str(&response).unwrap();
            let range = ExportRange{start_ms:500,end_ms:2500};
            let export = simulate_cursor_range_with_activity(&events,&spring,fps as f64,range,&None,
                &settings,None,Some(&typing));
            for (index, frame) in export.iter().enumerate() {
                assert_eq!(frame.opacity, preview[index + fps as usize / 2][2]);
                assert_eq!(frame.fade_blur_radius, preview[index + fps as usize / 2][8]);
            }
            assert!(preview[(0.8 * fps as f64) as usize][2] < 0.01);
            assert!(preview[(2.0 * fps as f64) as usize][2] > 0.95);
        }
        let frames = simulate_cursor_frames_with_activity(&events,&spring,120.0,2500,&None,
            &serde_json::from_value(serde_json::json!({"trail_enabled":true})).unwrap(),None,Some(&typing),Clone::clone);
        assert!(frames[96].trail_points().iter().all(|point| point.alpha < 0.01));
        std::fs::remove_file(path).unwrap();
    }
}
