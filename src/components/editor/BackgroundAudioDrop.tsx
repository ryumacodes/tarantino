import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { droppedAudioPath, importDroppedAudio } from '../../utils/audio/importDroppedAudio';
import { useEditorStore } from '../../stores/editor';

export default function BackgroundAudioDrop({disabled,onImported}:{disabled:boolean;onImported:()=>void}) {
  const latest=useRef({disabled,onImported});latest.current={disabled,onImported};
  const [hover,setHover]=useState(false),[busy,setBusy]=useState(false);
  const [notice,setNotice]=useState<{revision:number;message:string;error:boolean}|null>(null);
  const revision=useEditorStore(state=>state.recordingRevision);
  useEffect(()=>{
    if(!isTauri())return;
    let alive=true,importing=false;
    const listener=getCurrentWebview().onDragDropEvent(async({payload})=>{
      if(!alive)return;
      if(payload.type==='leave'){setHover(false);return;}
      if(payload.type==='enter'){
        setHover(!latest.current.disabled && !importing && !!droppedAudioPath(payload.paths));return;
      }
      if(payload.type!=='drop')return;
      setHover(false);
      if(latest.current.disabled || importing)return;
      const path=droppedAudioPath(payload.paths);if(!path)return;
      const recordingRevision=useEditorStore.getState().recordingRevision;
      importing=true;setBusy(true);setNotice(null);
      try {
        const applied=await importDroppedAudio(path,
          sourcePath=>invoke('import_background_audio_path',{sourcePath}),
          ()=>alive && !latest.current.disabled);
        if(applied){
          latest.current.onImported();
          setNotice({revision:recordingRevision,message:'Background music added.',error:false});
        }
      } catch(error) {
        if(alive && useEditorStore.getState().recordingRevision===recordingRevision)
          setNotice({revision:recordingRevision,message:String(error),error:true});
      } finally {importing=false;if(alive)setBusy(false);}
    });
    void listener.catch(error=>{if(alive)console.error('Audio drop listener unavailable:',error);});
    return ()=>{alive=false;void listener.then(unlisten=>unlisten()).catch(()=>{});};
  },[]);
  const visibleNotice=notice?.revision===revision ? notice : null;
  if(!hover && !busy && !visibleNotice)return null;
  return <div className="editor-export-status" data-error={visibleNotice?.error || undefined}>
    <span role={visibleNotice?.error ? 'alert':'status'}>
      {busy?'Importing background music…':hover && !disabled?'Drop to add background music':visibleNotice?.message}
    </span>
    {visibleNotice && !busy && <button type="button" onClick={()=>setNotice(null)}>Dismiss</button>}
  </div>;
}
