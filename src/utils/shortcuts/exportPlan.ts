import type {EditorState} from '../../stores/editor/types';
import {timelineDuration,visibleTimelineSegments} from '../timeline/previewTimeline';
import {shortcutAtOutputTime} from './timeline';
import type {ShortcutInterval} from './intervals';
export interface ShortcutSpan {start_frame:number;end_frame:number;tile:number}
export interface ShortcutPlan {frames:number;labels:ShortcutInterval[];spans:ShortcutSpan[]}

/** Partition only at edit/key boundaries; preparation cost is independent of duration. */
export function shortcutExportPlan(state:EditorState,fps:number):ShortcutPlan {
  const duration=Math.max(0,Math.min(state.trimEnd,timelineDuration(state))-state.trimStart);
  const frames=Math.max(1,Math.ceil(duration*fps/1000-1e-7));
  const plan:ShortcutPlan={frames,labels:[],spans:[]};
  if(!state.shortcutSettings.enabled||!state.shortcutIntervals.length||duration<=0)return plan;
  const hidden=new Set(state.shortcutSettings.hidden);
  const intervals=state.shortcutIntervals.filter(i=>!hidden.has(i.firstDown));
  const boundaries=new Set([0,frames]);
  const add=(time:number)=>{
    const frame=Math.ceil((time-state.trimStart)*fps/1000);
    // Include neighboring samples to preserve exact inclusive source endpoints
    // despite fractional speeds and floating-point clock conversion.
    for(const candidate of [frame-1,frame,frame+1])if(candidate>0&&candidate<frames)boundaries.add(candidate);
  };
  const legacy=!state.tracks.some(t=>t.type==='video'||t.type==='screen');
  const segments=legacy?[{start:0,end:state.duration,sourceIn:0,clipStart:0,rate:1,path:undefined}]:
    visibleTimelineSegments(state).map(({start,end,entry:{clip}})=>({start,end,sourceIn:clip.sourceIn,
      clipStart:clip.startTime,rate:clip.playbackRate,path:clip.sourceFilePath}));
  const sourceBoundaries=[...intervals.flatMap(interval=>[interval.start,interval.end]),
    ...(state.hasWebcam?state.webcamKeyframes.map(key=>key.time):[])];
  for(const segment of segments) {
    add(segment.start);add(segment.end);
    if(segment.path&&segment.path!==state.videoFilePath)continue;
    for(const sourceTime of sourceBoundaries) {
      const time=segment.clipStart+(sourceTime-segment.sourceIn)/segment.rate;
      if(time>=segment.start&&time<=segment.end)add(time);
    }
  }
  const times=[...boundaries].sort((a,b)=>a-b),tiles=new Map<string,number>();
  for(let index=0;index<times.length-1;index++) {
    const start=times[index],end=times[index+1];
    const interval=shortcutAtOutputTime(state,start*1000/fps,path=>
      !path||path===state.videoFilePath?{intervals}:undefined);
    if(!interval)continue;
    const identity=JSON.stringify([interval.key,interval.modifiers]);
    let tile=tiles.get(identity);
    if(tile===undefined){tile=plan.labels.length;tiles.set(identity,tile);plan.labels.push(interval);}
    const previous=plan.spans[plan.spans.length-1];
    if(previous?.tile===tile&&previous.end_frame===start)previous.end_frame=end;
    else plan.spans.push({start_frame:start,end_frame:end,tile});
  }
  return plan;
}
