import { peakIndex } from './peakIndex';
import { visibleTimelineSegments, type TimelineState } from '../timeline/previewTimeline';

/** Project source peak ranges onto visible pixels, retaining cut boundaries and clip gain. */
export function recordedWaveform(state:TimelineState,sourcePath:string|null,peaks:readonly number[],
  type:'microphone'|'system',pixelsPerMs:number,left:number,right:number,gain=1,rate=100){
  const start=Math.max(0,Math.floor(left)),end=Math.max(start,Math.ceil(right));
  if(!Number.isFinite(start+end+pixelsPerMs) || pixelsPerMs<=0)return new Float32Array();
  const columns=new Float32Array(end-start);
  const index=peakIndex(peaks);
  const ranges=state.tracks.some(t=>t.type==='video'||t.type==='screen')
    ? visibleTimelineSegments(state).map(s=>({...s,clip:s.entry.clip,muted:s.entry.track.muted}))
    : [{start:0,end:state.duration,clip:null,muted:false}];
  for(const range of ranges){
    const clip=range.clip;
    if(range.muted || (clip?.sourceFilePath && clip.sourceFilePath!==sourcePath))continue;
    const volume=gain*(clip ? (type==='microphone'?clip.micVolume:clip.systemVolume) ?? 1 : 1);
    if(volume<=0)continue;
    const x0=Math.max(start,Math.floor(range.start*pixelsPerMs));
    const x1=Math.min(end,Math.ceil(range.end*pixelsPerMs));
    for(let x=x0;x<x1;x++){
      const t0=Math.max(range.start,x/pixelsPerMs),t1=Math.min(range.end,(x+1)/pixelsPerMs);
      const source0=clip?clip.sourceIn+(t0-clip.startTime)*clip.playbackRate:t0;
      const source1=clip?Math.min(clip.sourceOut,clip.sourceIn+(t1-clip.startTime)*clip.playbackRate):t1;
      const peak=index.range(source0*rate/1000,source1*rate/1000);
      columns[x-start]=Math.max(columns[x-start],Math.min(1,peak*volume));
    }
  }
  return columns;
}
