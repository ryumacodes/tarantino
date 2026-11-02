import {maskPixelBounds} from './geometry';
import type {EditorState} from '../../stores/editor/types';
import {resolveSourceCrop} from '../sourceCrop';
import {timelineDuration} from '../timeline/previewTimeline';
import {maskOccurrences} from './timeline';
export function maskExportPayload(state:EditorState,fps:number) {
  if(!state.masks.some(mask=>!mask.disabled))return undefined;
  if(!Number.isFinite(fps)||fps<=0||fps>240)throw new Error('Invalid mask export frame rate');
  const width=state.videoWidth,height=state.videoHeight;
  if(!width||!height)throw new Error('Recording dimensions are required to export masks');
  const crop=resolveSourceCrop(state.visualSettings.sourceCrop??{x:0,y:0,width:1,height:1},width,height);
  const frames=Math.ceil(Math.max(0,Math.min(state.trimEnd,timelineDuration(state))-state.trimStart)*fps/1000);
  const spans=maskOccurrences(state,state.masks).map(({mask,startMs,endMs})=>{
    const basis=mask.coordinateSpace;
    // The decoder applies the primary recording's absolute crop to every clip.
    // Without a crop it scales each entire source, including differently sized clips.
    const target=basis&&!state.visualSettings.sourceCrop
      ?{x:0,y:0,width:basis.sourceWidth,height:basis.sourceHeight,sourceWidth:basis.sourceWidth,sourceHeight:basis.sourceHeight}:crop;
    const rect=maskPixelBounds(mask,target);
    return {start:Math.ceil(startMs*fps/1000),end:Math.ceil(endMs*fps/1000),
      rect:[rect.x/target.width,rect.y/target.height,rect.width/target.width,rect.height/target.height],
      radius:16/target.width,sensitive:mask.type==='sensitive-data',highlightOpacity:mask.highlightOpacity};
  }).filter(span=>span.end>span.start).sort((a,b)=>a.start-b.start);
  for(let i=1;i<spans.length;i++)if(spans[i].start<spans[i-1].end)throw new Error('Overlapping masks must be separated before export');
  return spans.length?{frames,spans}:undefined;
}
