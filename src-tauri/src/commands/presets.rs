//! Versioned style library with atomic saves and optimistic concurrency control.
use serde::{Deserialize,Serialize};
use serde_json::Value;
use std::{collections::HashSet,fs,io::{Read,Write},path::Path,sync::Mutex};
use tauri::Manager;
const MAX_BYTES:u64=4*1024*1024;
static LIBRARY_LOCK:Mutex<()>=Mutex::new(());
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PresetLibrary {
    version:u32,
    revision:u64,
    presets:Vec<Value>,
    default_id:Option<String>,
}
impl Default for PresetLibrary {
    fn default()->Self{Self{version:1,revision:0,presets:vec![],default_id:None}}
}
fn validate(library:&PresetLibrary)->Result<(),String>{
    if library.version!=1 || library.presets.len()>1000{return Err("Unsupported or oversized preset library".into());}
    let mut ids=HashSet::new();
    for preset in &library.presets {
        let id=preset["id"].as_str().filter(|id|!id.is_empty() && id.len()<=128).ok_or("Invalid preset identifier")?;
        let name=preset["name"].as_str().filter(|name|!name.trim().is_empty() && name.chars().count()<=120);
        if !ids.insert(id) || name.is_none() || preset["format"]!="tarantino-style" || preset["version"]!=1
            || !preset["visual"].is_object() || !preset["audio"].is_object() || !preset["output"].is_object(){
            return Err("Invalid or duplicate style preset".into());
        }
    }
    if library.default_id.as_ref().is_some_and(|id|!ids.contains(id.as_str())){return Err("Default preset is missing".into());}
    Ok(())
}
fn read(root:&Path)->Result<PresetLibrary,String>{
    let file=match fs::File::open(root.join("styles.json")){
        Ok(file)=>file,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(PresetLibrary::default()),
        Err(error)=>return Err(error.to_string()),
    };
    let mut bytes=Vec::new();file.take(MAX_BYTES+1).read_to_end(&mut bytes).map_err(|error|error.to_string())?;
    if bytes.len() as u64>MAX_BYTES{return Err("Preset library is too large".into());}
    let library=serde_json::from_slice(&bytes).map_err(|error|format!("Could not read preset library: {error}"))?;
    validate(&library)?;Ok(library)
}
fn save(root:&Path,mut library:PresetLibrary)->Result<PresetLibrary,String>{
    let _lock=LIBRARY_LOCK.lock().map_err(|_|"Preset library lock unavailable")?;
    validate(&library)?;
    let current=read(root)?;
    if current.revision!=library.revision{return Err("Preset library changed in another window. Reload before saving.".into());}
    library.revision=library.revision.checked_add(1).ok_or("Preset revision overflow")?;
    let bytes=serde_json::to_vec(&library).map_err(|error|error.to_string())?;
    if bytes.len() as u64>MAX_BYTES{return Err("Preset library is too large".into());}
    fs::create_dir_all(root).map_err(|error|error.to_string())?;
    let temporary=root.join(format!(".{}.tmp",uuid::Uuid::new_v4()));
    let result=(||->std::io::Result<()>{
        let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&bytes)?;file.sync_all()?;
        fs::rename(&temporary,root.join("styles.json"))?;
        Ok(())
    })();
    if result.is_err(){let _=fs::remove_file(&temporary);}
    result.map_err(|error|error.to_string())?;
    Ok(library)
}
#[tauri::command]
pub async fn load_style_presets(app:tauri::AppHandle)->Result<PresetLibrary,String>{
    let root=app.path().app_data_dir().map_err(|error|error.to_string())?.join("presets");
    tokio::task::spawn_blocking(move||read(&root)).await.map_err(|error|error.to_string())?
}
#[tauri::command]
pub async fn save_style_presets(app:tauri::AppHandle,library:PresetLibrary)->Result<PresetLibrary,String>{
    let root=app.path().app_data_dir().map_err(|error|error.to_string())?.join("presets");
    tokio::task::spawn_blocking(move||save(&root,library)).await.map_err(|error|error.to_string())?
}
fn export_to(root:&Path,preset:Value)->Result<String,String>{
    validate(&PresetLibrary{presets:vec![preset.clone()],..PresetLibrary::default()})?;
    let name=preset["name"].as_str().unwrap().chars().take(64)
        .map(|c|if c.is_alphanumeric() || c=='-' || c=='_' {c}else{'-'}).collect::<String>();
    let bytes=serde_json::to_vec_pretty(&preset).map_err(|error|error.to_string())?;
    if bytes.len() as u64>MAX_BYTES{return Err("Preset file is too large".into());}
    fs::create_dir_all(root).map_err(|error|error.to_string())?;
    let target=root.join(format!("{name}-{}.tarantino-style.json",uuid::Uuid::new_v4()));
    let temporary=root.join(format!(".{}.tmp",uuid::Uuid::new_v4()));
    let result=(||->std::io::Result<()>{
        let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&bytes)?;file.sync_all()?;fs::rename(&temporary,&target)?;Ok(())
    })();
    if result.is_err(){let _=fs::remove_file(&temporary);}
    result.map_err(|error|error.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}
#[tauri::command]
pub async fn export_style_preset(app:tauri::AppHandle,preset:Value)->Result<String,String>{
    let root=app.path().download_dir().map_err(|error|error.to_string())?.join("Tarantino Presets");
    tokio::task::spawn_blocking(move||export_to(&root,preset)).await.map_err(|error|error.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn preset(id:&str)->Value{serde_json::json!({"format":"tarantino-style","version":1,"id":id,"name":"Style","visual":{},"audio":{},"output":{}})}
    #[test]
    fn exports_portable_documents_without_filename_traversal_or_overwriting(){
        let root=std::env::temp_dir().join(format!("preset-export-test-{}",uuid::Uuid::new_v4()));
        let mut value=preset("one");value["name"]=Value::String("../../Example".into());
        value["visual"]["customBackgroundImage"]=Value::String("data:image/png;base64,aGVsbG8=".into());
        let first=export_to(&root,value.clone()).unwrap();let second=export_to(&root,value.clone()).unwrap();
        assert_ne!(first,second);assert_eq!(Path::new(&first).parent(),Some(root.as_path()));
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(first).unwrap()).unwrap(),value);
        assert_eq!(fs::read_dir(&root).unwrap().count(),2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn persists_updates_defaults_and_deletion_without_overwriting_stale_or_invalid_data(){
        let root=std::env::temp_dir().join(format!("presets-test-{}",uuid::Uuid::new_v4()));
        let empty=read(&root).unwrap();assert_eq!(empty.revision,0);
        let first=save(&root,PresetLibrary{presets:vec![preset("one")],default_id:Some("one".into()),..empty.clone()}).unwrap();
        assert_eq!(read(&root).unwrap().default_id.as_deref(),Some("one"));
        assert!(save(&root,empty).is_err());
        let original=fs::read(root.join("styles.json")).unwrap();
        let mut invalid=first.clone();invalid.presets.push(preset("one"));assert!(save(&root,invalid).is_err());
        let mut invalid=first.clone();invalid.default_id=Some("missing".into());assert!(save(&root,invalid).is_err());
        assert_eq!(fs::read(root.join("styles.json")).unwrap(),original);
        let mut updated=first.clone();updated.presets[0]["name"]=Value::String("Renamed".into());
        let renamed=save(&root,updated).unwrap();assert_eq!(renamed.revision,2);assert_eq!(read(&root).unwrap().presets[0]["name"],"Renamed");
        let deleted=save(&root,PresetLibrary{presets:vec![],default_id:None,..renamed}).unwrap();assert_eq!(deleted.revision,3);
        assert_eq!(fs::read_dir(&root).unwrap().count(),1);
        fs::write(root.join("styles.json"),b"broken").unwrap();assert!(save(&root,deleted).is_err());
        assert_eq!(fs::read(root.join("styles.json")).unwrap(),b"broken");
        fs::remove_dir_all(root).unwrap();
    }
}
