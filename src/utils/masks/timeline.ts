import {projectSourceIntervals,SourceIntervalIndex} from '../timeline/sourceIntervals';
import type {TimelineState} from '../timeline/previewTimeline';
import type {RecordingMask} from './document';

export interface MaskOccurrence {
  mask:RecordingMask;
  clipId:string|null;
  startMs:number;
  endMs:number;
  sourceStartMs:number;
  sourceEndMs:number;
}
/** Build on mask/timeline edits, not on every playback frame. Output is trim-relative. */
export function maskOccurrences(
  state:TimelineState&{trimStart:number;trimEnd:number;videoFilePath:string|null},
  masks:readonly RecordingMask[],
):MaskOccurrence[] {
  const bySource=new Map<string,{mask:RecordingMask;startMs:number;endMs:number}[]>();
  for(const mask of masks){
    if(mask.disabled)continue;
    let intervals=bySource.get(mask.sourceFilePath);
    if(!intervals){intervals=[];bySource.set(mask.sourceFilePath,intervals);}
    intervals.push({mask,startMs:mask.sourceStartMs,endMs:mask.sourceEndMs});
  }
  const indexes=new Map([...bySource].map(([path,intervals])=>[path,new SourceIntervalIndex(intervals)]));
  return projectSourceIntervals(state,path=>indexes.get(path??state.videoFilePath??'')).map(occurrence=>({
    mask:occurrence.word.mask,clipId:occurrence.clipId,startMs:occurrence.startMs,endMs:occurrence.endMs,
    sourceStartMs:occurrence.sourceStartMs,sourceEndMs:occurrence.sourceEndMs,
  }));
}
