//! Imported soundtracks are copied into owned local storage.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{path::{Path,PathBuf},time::Duration};
use tauri::Manager;
const MAX_BYTES:usize=64*1024*1024;
#[derive(Serialize)]
pub struct AudioAsset { pub path:String, pub name:String }
struct PendingAsset(PathBuf);
impl Drop for PendingAsset { fn drop(&mut self){let _=std::fs::remove_file(&self.0);} }

async fn import_asset(root:&Path,name:&str,data:&str)->Result<AudioAsset,String>{
    if data.len()>MAX_BYTES.div_ceil(3)*4 {return Err("Audio files must be 64 MB or smaller".into());}
    let bytes=STANDARD.decode(data).map_err(|_|"Invalid audio data")?;
    if bytes.is_empty() || bytes.len()>MAX_BYTES {return Err("Audio file is empty or too large".into());}
    let (name,extension)=asset_name(name)?;
    std::fs::create_dir_all(root).map_err(|error|error.to_string())?;
    let pending=PendingAsset(root.join(format!("{}.{}",uuid::Uuid::new_v4(),extension)));
    std::fs::write(&pending.0,bytes).map_err(|error|error.to_string())?;
    validate_asset(pending,name).await
}
fn asset_name(name:&str)->Result<(&str,String),String>{
    let name=Path::new(name).file_name().and_then(|name|name.to_str()).ok_or("Invalid audio filename")?;
    let extension=Path::new(name).extension().and_then(|ext|ext.to_str()).unwrap_or("").to_ascii_lowercase();
    if !["mp3","wav","m4a","aac","ogg","opus","flac","aif","aiff"].contains(&extension.as_str()) {
        return Err("Choose an MP3, WAV, M4A, AAC, OGG, Opus, FLAC or AIFF audio file".into());
    }
    Ok((name,extension))
}
async fn import_path(root:&Path,source:&Path)->Result<AudioAsset,String>{
    let (name,extension)=asset_name(source.to_str().ok_or("Invalid audio path")?)?;
    let metadata=tokio::fs::metadata(source).await.map_err(|error|error.to_string())?;
    if !metadata.is_file() || metadata.len()==0 {return Err("Choose a nonempty audio file".into());}
    tokio::fs::create_dir_all(root).await.map_err(|error|error.to_string())?;
    let pending=PendingAsset(root.join(format!("{}.{}",uuid::Uuid::new_v4(),extension)));
    // Native file copy avoids loading the entire soundtrack into WebView memory.
    tokio::fs::copy(source,&pending.0).await.map_err(|error|error.to_string())?;
    validate_asset(pending,name).await
}
async fn validate_asset(mut pending:PendingAsset,name:&str)->Result<AudioAsset,String>{
    let mut command=tokio::process::Command::new("ffprobe");
    command.kill_on_drop(true).args(["-v","error","-select_streams","a:0","-show_entries","stream=codec_type","-of","json"]).arg(&pending.0);
    let result=tokio::time::timeout(Duration::from_secs(15),command.output()).await
        .map_err(|_|"Audio validation timed out")?.map_err(|error|error.to_string())?;
    let metadata:serde_json::Value=serde_json::from_slice(&result.stdout).map_err(|_|"Could not read audio file")?;
    if !result.status.success() || metadata["streams"].as_array().is_none_or(|streams|streams.is_empty()) {
        return Err("The selected file has no readable audio track".into());
    }
    let asset=AudioAsset{path:pending.0.to_string_lossy().into_owned(),name:name.into()};
    // Successful imports remain available for saved projects and undo history.
    let _ = std::mem::take(&mut pending.0);
    Ok(asset)
}
#[tauri::command]
pub async fn import_background_audio(app:tauri::AppHandle,file_name:String,data_base64:String)->Result<AudioAsset,String>{
    let root=app.path().app_data_dir().map_err(|error|error.to_string())?.join("audio-assets");
    import_asset(&root,&file_name,&data_base64).await
}
#[tauri::command]
pub async fn import_background_audio_path(app:tauri::AppHandle,source_path:String)->Result<AudioAsset,String>{
    let root=app.path().app_data_dir().map_err(|error|error.to_string())?.join("audio-assets");
    import_path(&root,Path::new(&source_path)).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn validates_and_copies_soundtracks_without_retaining_failed_imports(){
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none(){return;}
        let root=std::env::temp_dir().join(format!("audio-assets-test-{}",uuid::Uuid::new_v4()));
        let output=std::process::Command::new("ffmpeg").args(["-v","error","-f","lavfi","-i","sine=frequency=440:duration=0.1","-f","wav","pipe:1"]).output().unwrap();
        assert!(output.status.success());
        let asset=import_asset(&root,"../../music.wav",&STANDARD.encode(&output.stdout)).await.unwrap();
        assert_eq!(asset.name,"music.wav");assert_eq!(Path::new(&asset.path).parent(),Some(root.as_path()));
        assert_eq!(std::fs::read(&asset.path).unwrap(),output.stdout);
        assert!(import_asset(&root,"bad.wav",&STANDARD.encode(b"invalid")).await.is_err());
        assert!(import_asset(&root,"bad.exe",&STANDARD.encode(b"invalid")).await.is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);
        let source=root.join("original.WAV");std::fs::write(&source,&output.stdout).unwrap();
        let copied=import_path(&root,&source).await.unwrap();
        std::fs::remove_file(&source).unwrap();
        assert_eq!(copied.name,"original.WAV");
        assert_eq!(std::fs::read(&copied.path).unwrap(),output.stdout);
        let bad=root.join("corrupt.wav");std::fs::write(&bad,b"invalid").unwrap();
        assert!(import_path(&root,&bad).await.is_err());std::fs::remove_file(bad).unwrap();
        assert!(import_path(&root,&source).await.is_err());
        assert!(import_path(&root,&root).await.is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
