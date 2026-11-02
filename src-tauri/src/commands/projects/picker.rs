//! Native package destination/source selection.
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackagePickerMode { Export, Import }

#[tauri::command]
pub async fn choose_project_package(app: tauri::AppHandle, mode: PackagePickerMode) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        use std::sync::atomic::Ordering;
        if OPEN.swap(true, Ordering::SeqCst) { return Err("A folder chooser is already open.".into()); }
        let (send, receive) = tokio::sync::oneshot::channel();
        let scheduled = app.run_on_main_thread(move || {
            let result = unsafe { macos_panel(mode) };
            OPEN.store(false, Ordering::SeqCst);
            let _ = send.send(result);
        });
        if let Err(error) = scheduled { OPEN.store(false, Ordering::SeqCst); return Err(error.to_string()); }
        receive.await.map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = (app, mode); Err("Enter the package folder path directly on this platform.".into()) }
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
unsafe fn macos_panel(mode: PackagePickerMode) -> Result<Option<String>, String> {
    use cocoa::{appkit::{NSOpenPanel, NSSavePanel, NSModalResponse}, base::{id, nil, YES, NO}, foundation::{NSString, NSAutoreleasePool}};
    use objc::{msg_send, sel, sel_impl};
    // AppKit panels and their autoreleased objects are confined to the main thread.
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let panel = match mode {
            PackagePickerMode::Import => {
                let panel = <id as NSOpenPanel>::openPanel(nil);
                panel.setCanChooseFiles_(NO);
                panel.setCanChooseDirectories_(YES);
                panel.setAllowsMultipleSelection_(NO);
                panel.setResolvesAliases_(YES);
                panel
            }
            PackagePickerMode::Export => {
                let panel = <id as NSSavePanel>::savePanel(nil);
                panel.setCanCreateDirectories(YES);
                let name = NSString::alloc(nil).init_str("Project.tarantino-project");
                let _: () = msg_send![panel, setNameFieldStringValue: name];
                let _: () = msg_send![name, release];
                panel
            }
        };
        let title = NSString::alloc(nil).init_str(match mode {
            PackagePickerMode::Export => "Export project package",
            PackagePickerMode::Import => "Choose project package folder",
        });
        let _: () = msg_send![panel, setTitle: title];
        let _: () = msg_send![title, release];
        let result = if panel.runModal() == NSModalResponse::NSModalResponseOk {
            let url = panel.URL();
            let path: id = msg_send![url, path];
            if path == nil { Err("The selected folder has no local path.".into()) }
            else {
                let bytes = path.UTF8String();
                if bytes.is_null() { Err("Could not read the selected folder path.".into()) }
                else { Ok(Some(std::ffi::CStr::from_ptr(bytes).to_string_lossy().into_owned())) }
            }
        } else { Ok(None) };
        pool.drain();
        result
    }
}
