import { useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { useEditGesture } from '../../../hooks/useEditGesture';

export default function BackgroundAudioControls({disabled=false}:{disabled?:boolean}) {
  const disabledRef=useRef(disabled);
  disabledRef.current=disabled;
  const music=useEditorStore(state=>state.audioSettings.backgroundAudio);
  const update=useEditorStore(state=>state.updateAudioSettings);
  const input=useRef<HTMLInputElement>(null);
  const [busy,setBusy]=useState(false),[error,setError]=useState('');
  const gesture=useEditGesture();
  const importFile=async(file:File)=>{
    if(disabled || busy)return;
    const revision=useEditorStore.getState().recordingRevision;
    setBusy(true);setError('');
    try {
      if(file.size>64*1024*1024)throw new Error('Audio files must be 64 MB or smaller.');
      const data=await new Promise<string>((resolve,reject)=>{
        const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);
        reader.onerror=()=>reject(new Error('Could not read the audio file.'));reader.readAsDataURL(file);
      });
      const asset=await invoke<{path:string;name:string}>('import_background_audio',{fileName:file.name,dataBase64:data});
      if(disabledRef.current || useEditorStore.getState().recordingRevision!==revision || useEditorStore.getState().audioSettings.backgroundAudio!==music)return;
      update({backgroundAudio:{...asset,volume:music?.volume ?? 1,muted:false}});
    } catch(error) {setError(String(error));}
    finally {setBusy(false);if(input.current)input.current.value='';}
  };
  return <div className="section">
    <div className="section-header"><h3>Background music</h3><p>Loops at normal speed across your edited video.</p></div>
    <input ref={input} type="file" accept=".mp3,.wav,.m4a,.aac,.ogg,.opus,.flac,.aif,.aiff" hidden
      onChange={event=>{const file=event.target.files?.[0];if(file)void importFile(file);}} />
    <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>input.current?.click()}>
      {busy?'Importing…':music?'Replace music':'Add music'}
    </button>
    {music && <>
      <p style={{overflowWrap:'anywhere'}}>{music.name}</p>
      <label className="checkbox-control"><input type="checkbox" checked={music.muted} disabled={disabled || busy}
        onChange={event=>update({backgroundAudio:{...music,muted:event.target.checked}})} />Mute background music</label>
      <div className="control-group"><label htmlFor="background-volume">Music volume</label>
        <div className="slider-control"><input id="background-volume" className="editor-slider" type="range" min="0" max="1" step="0.01"
          value={music.volume} disabled={disabled || busy} onPointerDown={gesture.begin} onPointerUp={gesture.end} onPointerCancel={gesture.end} onBlur={gesture.end}
          onKeyDown={gesture.begin} onKeyUp={gesture.end} onChange={event=>update({backgroundAudio:{...music,volume:Number(event.target.value)}})} />
          <span className="value-display">{Math.round(music.volume*100)}%</span></div>
      </div>
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>update({backgroundAudio:undefined})}>Remove music</button>
    </>}
    {error && <p role="alert">{error}</p>}
  </div>;
}
