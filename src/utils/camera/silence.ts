import { invoke } from '@tauri-apps/api/core';
import type { EditorState } from '../../stores/editor/types';
export interface SilenceRange {start_ms:number;end_ms:number}
const cache=new Map<string,Promise<SilenceRange[]>>();
export function inSilence(ranges:readonly SilenceRange[],time:number){
  let low=0,high=ranges.length;
  while(low<high){const middle=(low+high)>>1;if(ranges[middle].start_ms<=time)low=middle+1;else high=middle;}
  return low>0 && time<ranges[low-1].end_ms;
}
export function loadCameraSilence(state:Pick<EditorState,'videoFilePath'|'recordingRevision'|'hasMicrophone'|'hasWebcam'|'visualSettings'>):Promise<SilenceRange[]>{
  const minimumMs=state.visualSettings.webcamHideSilenceMs;
  if(!state.videoFilePath || !state.hasMicrophone || !state.hasWebcam || minimumMs==null)return Promise.resolve([]);
  const slash=state.videoFilePath.lastIndexOf('/');
  const audioPath=state.videoFilePath.slice(0,slash+1)+state.videoFilePath.slice(slash+1).replace(/^processed_/,'').replace(/\.[^/.]+$/,'')+'.mic.wav';
  const key=JSON.stringify([audioPath,state.recordingRevision,minimumMs]);
  let pending=cache.get(key);
  if(!pending){
    pending=invoke<SilenceRange[]>('detect_audio_silence',{audioPath,minimumMs,threshold:0.1}).catch(error=>{cache.delete(key);throw error;});
    cache.set(key,pending);
    if(cache.size>4)cache.delete(cache.keys().next().value!);
  }
  return pending;
}
