import { inSilence, type SilenceRange } from './silence';
import type { VisualSettings, WebcamKeyframe } from '../../stores/editor/types';
export type CameraLayout = 'overlay' | 'fullscreen' | 'hidden';
// Editor edits replace keyframe arrays. Weak keys let history/project eviction
// release their indexes without an ever-growing playback cache.
const schedules = new WeakMap<readonly WebcamKeyframe[], readonly WebcamKeyframe[]>();
function cameraKeyAt(keys: readonly WebcamKeyframe[], sourceTime: number) {
  let schedule = schedules.get(keys);
  if (!schedule) {
    schedule = [...keys].sort((a,b) => a.time-b.time);
    schedules.set(keys, schedule);
  }
  let low=0, high=schedule.length;
  while(low<high) {
    const middle=(low+high)>>1;
    if(schedule[middle].time<=sourceTime) low=middle+1; else high=middle;
  }
  return schedule[low-1];
}
export function sampleWebcam(visual: VisualSettings, keys: WebcamKeyframe[], sourceTime: number, silence:readonly SilenceRange[] = []) {
  // Last key at the same timestamp wins, matching stable native sorting.
  const key = cameraKeyAt(keys, sourceTime);
  const size=key?.size ?? visual.webcamSize;
  const half=size/2;
  const layout=key?.layout ?? (key ? 'overlay' : visual.webcamLayout ?? 'overlay');
  const x=Math.min(1-half,Math.max(half,key?.x ?? visual.webcamX ?? 0.895));
  const y=Math.min(1-half,Math.max(half,key?.y ?? visual.webcamY ?? 0.895));
  return { x:layout==='fullscreen'?0.5:x,y:layout==='fullscreen'?0.5:y,size:layout==='fullscreen'?1:size,
    shape:key?.shape ?? visual.webcamShape,visible:visual.webcamEnabled !== false && (key?.visible ?? true) && layout!=='hidden' && (layout==='fullscreen' || !inSilence(silence,sourceTime)),layout,keyframeId:key?.id };
}
