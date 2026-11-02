import {previewSample,timelineDuration,type TimelineState} from '../timeline/previewTimeline';
import type {TimedTranscriptText} from './timing';
import type {CaptionGroup} from './groups';
import type {CaptionMotionOptions} from './targets';
import type {CaptionPosition,CaptionRect} from './placement';
import {captionSceneTargets} from './sceneTargets';
import {CaptionActivity} from './activity';
import {CaptionMotion,type CaptionFrameTargets} from './motion';
export interface PreparedCaptionSource<T> {groups:readonly CaptionGroup<T>[];fontSize:number}
/** Prepared sources and edit arrays are immutable for this provider's lifetime.
 * Recreate after edits, as with the canonical preview timeline index. */
export function captionFrameProvider<T extends TimedTranscriptText>(
  state:TimelineState&{trimStart:number;trimEnd:number},primaryPath:string,
  resolve:(path:string)=>PreparedCaptionSource<T>|undefined,options:CaptionMotionOptions,
  stage:CaptionRect,position:CaptionPosition,
):(outputTime:number)=>CaptionFrameTargets|null {
  const indexes=new Map<PreparedCaptionSource<T>,CaptionActivity<T>>();
  const end=Math.min(state.trimEnd,timelineDuration(state));
  return outputTime=>{
    const time=state.trimStart+outputTime;
    if(!Number.isFinite(outputTime)||outputTime<0||!Number.isFinite(time)||time>=end)return null;
    const sample=previewSample(state,time);if(sample.sourceTime===null)return null;
    const sourceKey=sample.sourceFilePath??primaryPath,source=resolve(sourceKey);
    if(!source?.groups.length)return null;
    let index=indexes.get(source);
    if(!index){index=new CaptionActivity(source.groups);indexes.set(source,index);}
    return {sourceKey,...captionSceneTargets(source.groups,sample.sourceTime,options,stage,position,
      source.fontSize,index.at(sample.sourceTime))};
  };
}
export function createCaptionMotion<T extends TimedTranscriptText>(
  state:TimelineState&{trimStart:number;trimEnd:number},primaryPath:string,
  resolve:(path:string)=>PreparedCaptionSource<T>|undefined,options:CaptionMotionOptions,
  stage:CaptionRect,position:CaptionPosition,fps:number,
) {
  return new CaptionMotion(fps,captionFrameProvider(state,primaryPath,resolve,options,stage,position));
}
