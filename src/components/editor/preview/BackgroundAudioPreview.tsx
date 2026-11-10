import { subscribePreviewMute, usePreviewAudioStore } from '../../../stores/previewAudio';
import { useEffect, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { createBackgroundAudioPlayer } from '../../../utils/audio/backgroundAudio';

export default function BackgroundAudioPreview() {
  const path=useEditorStore(state=>state.audioSettings.backgroundAudio?.path);
  const revision=useEditorStore(state=>state.recordingRevision);
  const [failure,setFailure]=useState<{path:string;revision:number;message:string}|null>(null);
  const retry=useRef<()=>void>(()=>{});
  useEffect(()=>{
    if (!path) return;
    const audio=new Audio(convertFileSrc(path));
    audio.preload='auto';
    const player=createBackgroundAudioPlayer(audio,error=>{
      setFailure(error ? {path,revision,message:error instanceof Error && error.name==='NotAllowedError'
        ? 'Background music playback was blocked. Click Retry to enable it.'
        : error instanceof Error ? error.message : 'Background music preview is unavailable.'} : null);
    });
    const sync=(retry=false)=>{
      const state=useEditorStore.getState();
      const settings=state.audioSettings.backgroundAudio;
      if(settings?.path===path)player.sync(state.currentTime,state.isPlaying,{...settings,muted:settings.muted || usePreviewAudioStore.getState().muted},retry,state.playbackRevision);
    };
    retry.current=()=>{player.retry();sync(true);};
    const ready=()=>sync();
    const gesture=()=>sync(true);
    const unsubscribe=useEditorStore.subscribe(()=>sync());
    const unsubscribeMute=subscribePreviewMute(()=>sync());
    audio.addEventListener('loadedmetadata',ready);
    document.addEventListener('pointerdown',gesture);
    document.addEventListener('keydown',gesture);
    sync();
    return ()=>{
      retry.current=()=>{};
      unsubscribeMute();unsubscribe();audio.removeEventListener('loadedmetadata',ready);
      document.removeEventListener('pointerdown',gesture);document.removeEventListener('keydown',gesture);
      player.dispose();
    };
  },[path,revision]);
  if(!failure || failure.path!==path || failure.revision!==revision)return null;
  return <div className="editor-export-status" data-error>
    <span role="alert">{failure.message}</span>
    <button type="button" onClick={()=>retry.current()}>Retry music preview</button>
  </div>;
}
