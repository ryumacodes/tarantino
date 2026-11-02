//! Cancellable, bounded-memory package copying.
use serde::Serialize;
use std::{collections::HashMap, io::{Read, Write}, path::Path, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use tauri::ipc::Channel;

#[derive(Clone, Serialize)]
pub struct CopyProgress {
    pub file_name: String,
    pub file_bytes: u64,
    pub file_total: u64,
}
pub struct CopyControl {
    cancelled: AtomicBool,
    report: Box<dyn Fn(CopyProgress) + Send + Sync>,
}
impl Default for CopyControl {
    fn default() -> Self { Self { cancelled: AtomicBool::new(false), report: Box::new(|_| {}) } }
}
impl CopyControl {
    #[cfg(test)]
    pub fn cancel_on_first_progress() -> Arc<Self> {
        Arc::new_cyclic(|weak: &std::sync::Weak<Self>| {
            let weak=weak.clone();Self { cancelled:AtomicBool::new(false), report:Box::new(move |_| {
                weak.upgrade().unwrap().cancelled.store(true,Ordering::Relaxed);
            }) }
        })
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) { Err("Package copy cancelled.".into()) } else { Ok(()) }
    }
    pub fn copy(&self, source: &Path, target: &Path) -> Result<(), String> {
        self.copy_impl(source,target,true)
    }
    fn copy_impl(&self, source: &Path, target: &Path, allow_clone:bool) -> Result<(), String> {
        self.check()?;
        let mut input=std::fs::File::open(source).map_err(|e|e.to_string())?;
        let total=input.metadata().map_err(|e|e.to_string())?.len();
        let mut progress=CopyProgress { file_name:source.file_name().unwrap_or_default().to_string_lossy().into_owned(),file_bytes:0,file_total:total };
        (self.report)(progress.clone());
        self.check()?;
        if allow_clone && clone_file(source,target) {
            progress.file_bytes=total;
            (self.report)(progress);
            self.check()?;
            return std::fs::File::open(target).and_then(|file|file.sync_all()).map_err(|e|e.to_string());
        }
        let mut output=std::fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|e|e.to_string())?;
        let mut last=Instant::now();
        let mut buffer=vec![0u8;1024*1024];
        loop {
            self.check()?;
            let count=input.read(&mut buffer).map_err(|e|e.to_string())?;
            if count==0 { break; }
            output.write_all(&buffer[..count]).map_err(|e|e.to_string())?;
            progress.file_bytes+=count as u64;
            if progress.file_bytes == count as u64 || last.elapsed()>=Duration::from_millis(100) || progress.file_bytes>=total {
                (self.report)(progress.clone()); last=Instant::now();
            }
        }
        self.check()?;
        output.sync_all().map_err(|e|e.to_string())
    }
}
// Preserve APFS copy-on-write speed; cross-volume/unsupported copies use the cancellable stream.
#[cfg(target_os="macos")]
fn clone_file(source:&Path,target:&Path)->bool {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" { fn clonefile(source:*const std::ffi::c_char,target:*const std::ffi::c_char,flags:i32)->i32; }
    let (Ok(source),Ok(target))=(std::ffi::CString::new(source.as_os_str().as_bytes()),std::ffi::CString::new(target.as_os_str().as_bytes())) else {return false;};
    // With flags=0 clonefile follows the source, never replaces a destination, and fails atomically.
    unsafe { clonefile(source.as_ptr(),target.as_ptr(),0)==0 }
}
#[cfg(not(target_os="macos"))]
fn clone_file(_source:&Path,_target:&Path)->bool {false}

type Registry = HashMap<String, (String, Arc<CopyControl>)>;
static JOBS: std::sync::LazyLock<parking_lot::Mutex<Registry>> = std::sync::LazyLock::new(Default::default);
pub struct CopyJob { id:String, pub control:Arc<CopyControl> }
impl CopyJob {
    pub fn start(id:String, owner:String, channel:Channel<CopyProgress>) -> Result<Self,String> {
        uuid::Uuid::parse_str(&id).map_err(|_|"Invalid copy identifier")?;
        let mut jobs=JOBS.lock();
        if jobs.contains_key(&id) || jobs.values().any(|(window,_)|window==&owner) { return Err("A package copy is already running in this window.".into()); }
        let control=Arc::new(CopyControl { cancelled:AtomicBool::new(false),report:Box::new(move |progress| { let _=channel.send(progress); }) });
        jobs.insert(id.clone(),(owner,control.clone()));
        Ok(Self {id,control})
    }
}
impl Drop for CopyJob { fn drop(&mut self) { JOBS.lock().remove(&self.id); } }
#[tauri::command]
pub fn cancel_project_package(window:tauri::WebviewWindow, operation_id:String) -> Result<(),String> {
    let jobs=JOBS.lock();
    if let Some((owner,control))=jobs.get(&operation_id) {
        if owner!=window.label() { return Err("This copy belongs to another window.".into()); }
        control.cancelled.store(true,Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_exact_bytes_and_reports_file_progress() {
        let root=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());std::fs::create_dir(&root).unwrap();
        let bytes=vec![43u8;2*1024*1024+7];std::fs::write(root.join("source"),&bytes).unwrap();
        let progress=Arc::new(parking_lot::Mutex::new(Vec::new()));let events=progress.clone();
        let control=CopyControl {cancelled:AtomicBool::new(false),report:Box::new(move |value|events.lock().push(value))};
        control.copy_impl(&root.join("source"),&root.join("copy"),false).unwrap();
        assert_eq!(std::fs::read(root.join("copy")).unwrap(),bytes);
        let events=progress.lock();assert_eq!(events[0].file_bytes,0);assert_eq!(events.last().unwrap().file_bytes,bytes.len() as u64);
        assert!(events.iter().all(|event|event.file_total==bytes.len() as u64));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fast_copy_remains_independent_of_source_changes() {
        let root=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("source"),b"original").unwrap();
        CopyControl::default().copy(&root.join("source"),&root.join("copy")).unwrap();
        std::fs::write(root.join("source"),b"changed").unwrap();
        assert_eq!(std::fs::read(root.join("copy")).unwrap(),b"original");
        assert!(CopyControl::default().copy(&root.join("source"),&root.join("copy")).is_err());
        assert_eq!(std::fs::read(root.join("copy")).unwrap(),b"original");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_stops_before_next_chunk() {
        let root=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("source"),vec![9u8;3*1024*1024]).unwrap();
        let control:Arc<CopyControl>=Arc::new_cyclic(|weak: &std::sync::Weak<CopyControl>| {
            let weak=weak.clone();CopyControl {cancelled:AtomicBool::new(false),report:Box::new(move |progress| { if progress.file_bytes > 0 { weak.upgrade().unwrap().cancelled.store(true,Ordering::Relaxed); } })}
        });
        assert!(control.copy_impl(&root.join("source"),&root.join("copy"),false).unwrap_err().contains("cancelled"));
        assert_eq!(std::fs::metadata(root.join("copy")).unwrap().len(),1024*1024);
        assert_eq!(std::fs::metadata(root.join("source")).unwrap().len(),3*1024*1024);
        std::fs::remove_dir_all(root).unwrap();
    }
}
