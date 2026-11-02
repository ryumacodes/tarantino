import {useMaskEditor} from '../../../stores/maskEditor';
import {useEffect,useState} from 'react';
import {useShallow} from 'zustand/react/shallow';
import {invoke} from '@tauri-apps/api/core';
import {useEditorStore} from '../../../stores/editor';
import {previewSample} from '../../../utils/timeline/previewTimeline';
import {maskOccurrences} from '../../../utils/masks/timeline';
import {resolveSourceCrop} from '../../../utils/sourceCrop';
import type {RecordingMask} from '../../../utils/masks/document';
import type {MaskEdit} from '../../../utils/masks/editing';
import {useEditGesture} from '../../../hooks/useEditGesture';
import type {TabProps} from './types';
import './masks.css';

function NumberField({label,value,disabled,onCommit}:{label:string;value:number;disabled:boolean;onCommit:(value:number)=>void}){
 const display=String(Number(value.toFixed(3)));
 const [draft,setDraft]=useState(display);useEffect(()=>setDraft(display),[display]);
 const commit=()=>{const next=Number(draft);if(draft!==display&&draft.trim()&&Number.isFinite(next))onCommit(next);setDraft(display);};
 return <label>{label}<input type="number" step="any" value={draft} disabled={disabled}
  onChange={event=>setDraft(event.target.value)} onBlur={commit}
  onKeyDown={event=>{if(event.key==='Enter'){event.stopPropagation();event.currentTarget.blur();}}}/></label>;
}
export default function MasksTab({isExporting=false}:TabProps){
 const {masks,path,revision}=useEditorStore(useShallow(state=>({masks:state.masks,path:state.videoFilePath,revision:state.recordingRevision})));
 const selected=useMaskEditor(state=>state.selectedId),setSelected=useMaskEditor(state=>state.select);
 const drawing=useMaskEditor(state=>state.draw);
 const [error,setError]=useState(''),[pending,setPending]=useState(false);
 const gesture=useEditGesture();
 useEffect(()=>{setSelected(null);setError('');setPending(false);return()=>setSelected(null);},[path,revision,setSelected]);
 useEffect(()=>{if(isExporting)setSelected(null);},[isExporting,setSelected]);
 const mask=masks.find(item=>item.id===selected),disabled=isExporting||pending;
 const run=(action:()=>void)=>{try{action();setError('');}catch(error){
  const message=error instanceof Error?error.message:String(error);
  setError(message.startsWith('Invalid project: masks')?'Use positive dimensions and valid mask settings.':message);
 }};
 const focus=(item:RecordingMask,midpoint=false)=>{
  setSelected(item.id);
  const state=useEditorStore.getState(),sample=previewSample(state,state.currentTime);
  const occurrences=maskOccurrences(state,[{...item,disabled:false}]);
  const occurrence=occurrences.find(occurrence=>occurrence.clipId===sample.clipId)??occurrences[0];
  if(occurrence&&!state.isPlaying)state.setCurrentTime(state.trimStart+occurrence.startMs+(midpoint?(occurrence.endMs-occurrence.startMs)/2:0));
 };
 const add=async(type:RecordingMask['type'])=>{
  const original=useEditorStore.getState(),sample=previewSample(original,original.currentTime);
  const sourcePath=sample.sourceFilePath??original.videoFilePath;
  if(sample.sourceTime===null||!sourcePath){setError('Move the playhead onto a recording first.');return;}
  const existing=original.masks.find(item=>item.sourceFilePath===sourcePath&&item.sourceStartMs<=sample.sourceTime!&&item.sourceEndMs>sample.sourceTime!);
  if(existing){setSelected(existing.id);return;}
  setPending(true);setError('');
  try{
   const metadata=sourcePath===original.videoFilePath&&original.videoWidth&&original.videoHeight
    ?{width:original.videoWidth,height:original.videoHeight,duration_ms:original.duration}
    :await invoke<{width:number;height:number;duration_ms:number}>('get_video_metadata',{filePath:sourcePath});
   const state=useEditorStore.getState();
   if(state.recordingRevision!==original.recordingRevision||state.videoFilePath!==original.videoFilePath
     ||state.currentTime!==original.currentTime||state.clips!==original.clips||state.visualSettings.sourceCrop!==original.visualSettings.sourceCrop){
    setError('The recording or playhead changed. Add the mask at its new position.');return;
   }
   const base=state.visualSettings.sourceCrop&&state.videoWidth&&state.videoHeight
    ?resolveSourceCrop(state.visualSettings.sourceCrop,state.videoWidth,state.videoHeight)
    :resolveSourceCrop({x:0,y:0,width:1,height:1},metadata.width,metadata.height);
   const coordinateSpace={...base,sourceWidth:metadata.width,sourceHeight:metadata.height};
   const result=state.addMask({sourceFilePath:sourcePath,sourceTimeMs:sample.sourceTime,sourceDurationMs:metadata.duration_ms,
    coordinateSpace,type});
   focus(result.mask,result.created);
  }catch(error){setError(error instanceof Error?error.message:String(error));}finally{setPending(false);}
 };
 const update=(patch:MaskEdit)=>{if(mask)run(()=>useEditorStore.getState().updateMask(mask.id,patch));};
 return <div className="tab-content mask-controls" tabIndex={0} onKeyDown={event=>{
  if(disabled||!mask||event.metaKey||event.ctrlKey||event.altKey||(event.target as HTMLElement).closest('input,select,textarea,button,[contenteditable=true]'))return;
  if(event.key==='d'||event.key==='D'){event.preventDefault();update({disabled:!mask.disabled});}
  if(event.key==='Backspace'){event.preventDefault();run(()=>useEditorStore.getState().removeMask(mask.id));setSelected(null);}
  if(event.key==='Enter')setSelected(null);
 }}>
  <section className="section">
   <div className="section-header"><h3>Cover or highlight</h3><p>Attach a mask to the recording at the playhead.</p></div>
   <div className="mask-actions"><button className="editor-btn editor-btn--secondary" disabled={disabled||!path} onClick={()=>void add('sensitive-data')}>Cover sensitive area</button>
    <button className="editor-btn editor-btn--secondary" disabled={disabled||!path} onClick={()=>void add('highlight')}>Highlight area</button></div>
   {error&&<p className="setting-hint" role="alert">{error}</p>}
   {!masks.length&&<p className="setting-hint">No masks yet. Add one, then adjust its area and timing below.</p>}
   <div className="mask-list" aria-label="Masks">{masks.map((item,index)=><button key={item.id} disabled={disabled}
    className={`mask-item ${item.id===selected?'is-selected':''}`} aria-pressed={item.id===selected} onClick={()=>focus(item,true)}>
    <span>{item.type==='highlight'?'Highlight':'Sensitive area'} {index+1}{item.disabled?' · Disabled':''}</span>
    <small>{(item.sourceStartMs/1000).toFixed(1)}–{(item.sourceEndMs/1000).toFixed(1)} s</small></button>)}</div>
  </section>
  {mask&&<section className="section" key={mask.id}>
   <div className="section-header"><h3>Mask settings</h3><p>Position and size use pixels in the recording crop.</p></div>
   <div className="control-group"><label>Type<select value={mask.type} disabled={disabled} onChange={event=>update({type:event.target.value as RecordingMask['type']})}>
    <option value="sensitive-data">Sensitive area</option><option value="highlight">Highlight</option></select></label></div>
   <div className="control-group"><label><input type="checkbox" disabled={disabled} checked={!mask.disabled} onChange={event=>update({disabled:!event.target.checked})}/> Enabled</label></div>
   <div className="mask-actions"><button className="speed-preset-btn" disabled={disabled} aria-pressed={drawing}
    onClick={()=>drawing?useMaskEditor.getState().finishDrawing():useMaskEditor.getState().startDrawing()}>{drawing?'Cancel drawing':'Draw area on preview'}</button></div>
   <p className="setting-hint">{drawing?'Drag across the preview to replace the area.':'Drag inside the outline to move it, or drag a corner to resize.'}</p>
   <div className="mask-fields">{(['x','y','width','height'] as const).map(key=><NumberField key={key} label={{x:'Left',y:'Top',width:'Width',height:'Height'}[key]}
    value={mask.bounds[0][key]} disabled={disabled} onCommit={value=>update({bounds:[{...mask.bounds[0],[key]:value},...mask.bounds.slice(1)]})}/>)}</div>
   <div className="mask-fields"><NumberField label="Start (seconds)" value={mask.sourceStartMs/1000} disabled={disabled} onCommit={value=>update({sourceStartMs:value*1000})}/>
    <NumberField label="End (seconds)" value={mask.sourceEndMs/1000} disabled={disabled} onCommit={value=>update({sourceEndMs:value*1000})}/></div>
   {mask.type==='highlight'&&<div className="control-group"><label>Outside dimming: {Math.round(mask.highlightOpacity*100)}%
    <input aria-label="Outside dimming" type="range" min={10} max={90} value={mask.highlightOpacity*100} disabled={disabled}
     onPointerDown={gesture.begin} onPointerUp={gesture.end} onPointerCancel={gesture.end} onBlur={gesture.end}
     onChange={event=>update({highlightOpacity:Number(event.target.value)/100})}/></label>
    <div className="mask-actions"><button className="speed-preset-btn" disabled={disabled} onClick={()=>update({highlightOpacity:.5})}>Reset dimming</button>
     <button className="speed-preset-btn" disabled={disabled} onClick={()=>run(()=>useEditorStore.getState().applyMaskOpacityToAll(mask.id))}>Apply to all masks</button></div></div>}
   <div className="mask-actions"><button className="speed-preset-btn" disabled={disabled} onClick={()=>focus(mask)}>Go to start</button>
    <button className="speed-preset-btn" disabled={disabled} onClick={()=>{run(()=>useEditorStore.getState().removeMask(mask.id));setSelected(null);}}>Remove mask</button>
    <button className="speed-preset-btn" onClick={()=>setSelected(null)}>Done</button></div>
  </section>}
  {masks.length>0&&<section className="section mask-actions"><button className="speed-preset-btn" disabled={disabled} onClick={()=>run(()=>useEditorStore.getState().disableAllMasks())}>Disable all</button>
   <button className="speed-preset-btn" disabled={disabled} onClick={()=>{run(()=>useEditorStore.getState().removeAllMasks());setSelected(null);}}>Remove all</button></section>}
 </div>;
}
