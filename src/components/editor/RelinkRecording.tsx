import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export default function RelinkRecording({projectId,onRelinked}:{projectId:string;onRelinked:()=>void}) {
  const [path,setPath]=useState('');
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const pending=useRef(false);
  const alive=useRef(true);
  useEffect(()=>{
    alive.current=true;
    const listener=getCurrentWebview().onDragDropEvent(({payload})=>{
      if(payload.type!=='drop' || pending.current || !alive.current)return;
      if(payload.paths.length!==1){setError('Drop one recording file.');return;}
      setPath(payload.paths[0]);setError('');
    });
    void listener.catch(()=>{});
    return ()=>{alive.current=false;void listener.then(unlisten=>unlisten()).catch(()=>{});};
  },[]);
  const relink=async()=>{
    if(!path.trim() || pending.current)return;
    pending.current=true;setBusy(true);setError('');
    try {
      await invoke('relink_project_recording',{projectId,recordingPath:path.trim()});
      if(alive.current)onRelinked();
    } catch(error) {if(alive.current)setError(String(error));}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  };
  return <section style={{width:'min(440px,90vw)',display:'grid',gap:12}} aria-label="Find moved recording">
    <h2 style={{fontSize:18,margin:0}}>Find your recording</h2>
    <p style={{fontSize:14,margin:0}}>Drop the original video here, or paste its new file path. Keep its audio, camera, and mouse files alongside it.</p>
    <label>Recording file
      <input style={{width:'100%',marginTop:6}} aria-label="Replacement recording path" value={path}
        disabled={busy} onChange={event=>setPath(event.target.value)} placeholder="/path/to/recording.mp4" />
    </label>
    <button className="editor-btn editor-btn--primary" disabled={busy || !path.trim()} onClick={()=>void relink()}>
      {busy?'Checking recording…':'Relink and open'}
    </button>
    {error && <p role="alert">{error}</p>}
  </section>;
}
