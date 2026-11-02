//! AppKit image sampling stays on the main thread, outside the input event tap.
use crate::input::{cursor_images::CursorImage, MouseTracker};
use parking_lot::Mutex;
#[path = "cursor_bitmap.rs"]
mod bitmap;
use std::{sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};

pub fn install(app: &tauri::AppHandle, tracker: Arc<Mutex<MouseTracker>>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut clock = tokio::time::interval(Duration::from_secs_f64(1.0 / 60.0));
        clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut previous = None;
        let mut session = None;
        loop {
            clock.tick().await;
            let current = { let state = tracker.lock();
                state.is_tracking.then_some(state.recording_start_time).flatten() };
            if current != session { previous = None; session = current; }
            let Some(session_start) = current else { continue; };
            let tracker = tracker.clone();
            let (send, receive) = tokio::sync::oneshot::channel();
            if app.run_on_main_thread(move || {
                // Do not hold the tracker lock while calling AppKit.
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                    .unwrap_or_default().as_millis() as u64;
                let sample = unsafe { sample(previous) };
                let mut next = previous;
                if let Some((fingerprint, image)) = sample {
                    let mut state = tracker.lock();
                    if state.is_tracking && state.recording_start_time == Some(session_start) {
                        if let Some(image) = image { state.cursor_images.record(timestamp, image); }
                        next = Some(fingerprint);
                    }
                }
                let _ = send.send(next);
            }).is_err() { break; }
            // At most one outstanding query, even when the UI thread is busy.
            match receive.await { Ok(next) => previous = next, Err(_) => break }
        }
    });
}

#[allow(deprecated, unexpected_cfgs)]
unsafe fn sample(previous: Option<u64>) -> Option<(u64, Option<CursorImage>)> {
    use base64::Engine;
    use cocoa::{base::{id, nil}, foundation::{NSAutoreleasePool}};
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let result = (|| {
            let cursor: id = msg_send![class!(NSCursor), currentSystemCursor];
            let bitmap::Bitmap { rep, size, hotspot, fingerprint } = bitmap::read(cursor)?;
            if previous == Some(fingerprint) { return Some((fingerprint, None)); }
            let properties: id = msg_send![class!(NSDictionary), dictionary];
            // NSBitmapImageFileTypePNG = 4; encode only changed appearances.
            let png: id = msg_send![rep, representationUsingType: 4usize properties: properties];
            if png == nil { return None; }
            let bytes: *const u8 = msg_send![png, bytes];
            let length: usize = msg_send![png, length];
            if bytes.is_null() || length == 0 { return None; }
            let png_base64 = base64::engine::general_purpose::STANDARD
                .encode(std::slice::from_raw_parts(bytes,length));
            Some((fingerprint, Some(CursorImage {system_cursor:bitmap::identify(fingerprint),width:size.width,height:size.height,
                hotspot_x:hotspot.x,hotspot_y:hotspot.y,png_base64})))
        })();
        pool.drain();
        result
    }
}
