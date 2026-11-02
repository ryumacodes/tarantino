import { useCameraSilence } from '../../../hooks/useCameraSilence';
import { useShallow } from 'zustand/react/shallow';
import React, { useState } from 'react';
import { useEditorStore } from '../../../stores/editor';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { projectSourceRange } from '../../../utils/timeline/sourceRanges';
import type { CameraLayout } from '../../../utils/camera/webcamTimeline';

export function CameraTimelineControls({disabled=false}:{disabled?:boolean}) {
  const state=useEditorStore(useShallow(state => ({
    webcamKeyframes: state.webcamKeyframes, visualSettings: state.visualSettings,
    updateVisualSettings: state.updateVisualSettings, updateWebcamKeyframe: state.updateWebcamKeyframe,
    deleteWebcamKeyframe: state.deleteWebcamKeyframe,
  })));
  const silence=useCameraSilence();
  const hasMic=useEditorStore(s=>s.hasMicrophone);
  const [selected,setSelected]=useState<string|null>(null);
  const key=state.webcamKeyframes.find(k=>k.id===selected);
  const canSave=useEditorStore(state => previewSample(state,state.currentTime).sourceTime !== null);
  const visual=state.visualSettings;
  const save=() => {
    const state=useEditorStore.getState();
    const source=previewSample(state,state.currentTime).sourceTime;
    if (source===null) return;
    const time=Math.round(source);
    const existing=state.webcamKeyframes.find(k=>k.time===time);
    const id=existing?.id ?? crypto.randomUUID();
    const value={id,time,x:visual.webcamX??0.895,y:visual.webcamY??0.895,size:visual.webcamSize,
      shape:visual.webcamShape,layout:visual.webcamLayout??'overlay',visible:visual.webcamLayout!=='hidden'};
    if (existing) state.updateWebcamKeyframe(id,value); else state.addWebcamKeyframe(value);
    setSelected(id);
  };
  return <div className="cursor-setting-section">
    <label className="checkbox-control"><input type="checkbox" checked={visual.webcamMirrored ?? false} disabled={disabled}
      onChange={e=>state.updateVisualSettings({webcamMirrored:e.target.checked})}/>Mirror camera</label>
    <label className="checkbox-control"><input type="checkbox" checked={visual.webcamHideSilenceMs!=null} disabled={disabled || !hasMic}
      onChange={e=>state.updateVisualSettings({webcamHideSilenceMs:e.target.checked?5000:null})}/>Hide camera during silence</label>
    {!hasMic && <p>Record a microphone track to use silence detection.</p>}
    {visual.webcamHideSilenceMs!=null && <label className="cursor-setting-label">Minimum silence (seconds)
      <input type="number" min="0.1" max="60" step="0.1" disabled={disabled} value={visual.webcamHideSilenceMs/1000}
        onChange={e=>{const seconds=Number(e.target.value);if(Number.isFinite(seconds) && seconds>=0.1 && seconds<=60)state.updateVisualSettings({webcamHideSilenceMs:seconds*1000});}}/></label>}
    {silence.loading && <p role="status">Analyzing microphone silence…</p>}
    {silence.error && <p role="alert">Silence detection failed: {silence.error}. Turn the option off and on to retry.</p>}
    <label className="cursor-setting-label">Default camera layout</label>
    <select className="editor-input" value={visual.webcamLayout??'overlay'} disabled={disabled}
      onChange={e=>state.updateVisualSettings({webcamLayout:e.target.value as CameraLayout})}>
      <option value="overlay">Overlay</option><option value="fullscreen">Fullscreen camera</option><option value="hidden">Hidden</option>
    </select>
    <button className="editor-btn" disabled={disabled||!canSave} onClick={save}>Save default camera at playhead</button>
    <label className="cursor-setting-label">Camera changes</label>
    <select className="editor-input" value={selected??''} disabled={disabled} onChange={e=>{
      setSelected(e.target.value||null);
      const state=useEditorStore.getState();
      const chosen=state.webcamKeyframes.find(k=>k.id===e.target.value);
      if (!chosen) return;
      const occurrences=projectSourceRange(state,chosen.time,chosen.time+0.001);
      occurrences.sort((a,b)=>Math.abs(a.startTime-state.currentTime)-Math.abs(b.startTime-state.currentTime));
      if (occurrences[0]) state.setCurrentTime(occurrences[0].startTime);
    }}>
      <option value="">Select a change</option>
      {[...state.webcamKeyframes].sort((a,b)=>a.time-b.time).map(k=><option key={k.id} value={k.id}>{(k.time/1000).toFixed(2)}s recorded · {k.visible?k.layout??'overlay':'hidden'}</option>)}
    </select>
    {key && <>
      <select className="editor-input" aria-label="Camera change layout" disabled={disabled} value={key.visible?key.layout??'overlay':'hidden'}
        onChange={e=>state.updateWebcamKeyframe(key.id,{layout:e.target.value as CameraLayout,visible:e.target.value!=='hidden'})}>
        <option value="overlay">Overlay</option><option value="fullscreen">Fullscreen camera</option><option value="hidden">Hidden</option>
      </select>
      {(['x','y','size'] as const).map(field=><label key={field} className="cursor-setting-label">
        {field==='x'?'Horizontal':field==='y'?'Vertical':'Size'}
        <input type="range" min={field==='size'?0.08:0} max={field==='size'?0.5:1} step="0.01" value={key[field]} disabled={disabled}
          onChange={e=>state.updateWebcamKeyframe(key.id,{[field]:Number(e.target.value)})}/>
      </label>)}
      <select aria-label="Camera change shape" className="editor-input" disabled={disabled} value={key.shape}
        onChange={e=>state.updateWebcamKeyframe(key.id,{shape:e.target.value as 'circle'|'roundrect'})}>
        <option value="circle">Circle</option><option value="roundrect">Rounded rectangle</option>
      </select>
      <button className="editor-btn" disabled={disabled} onClick={()=>{state.deleteWebcamKeyframe(key.id);setSelected(null);}}>Delete camera change</button>
    </>}
  </div>;
}
