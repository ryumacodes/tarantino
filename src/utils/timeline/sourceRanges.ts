import { previewSample, timelineDuration, visibleTimelineSegments, type TimelineState } from './previewTimeline';

export interface SourceRangeView {
  occurrenceId: string;
  startTime: number; endTime: number;
  sourceStart: number; sourceEnd: number;
  originalStart: number; originalEnd: number;
  rate: number;
}
/** One source range can appear more than once after clips are duplicated/reordered. */
export function projectSourceRange(state: TimelineState, start: number, end: number): SourceRangeView[] {
  if (end <= start) return [];
  if (!state.tracks.some(t => t.type === 'video' || t.type === 'screen')) {
    return [{ occurrenceId:'source',startTime:start,endTime:end,sourceStart:start,sourceEnd:end,
      originalStart:start,originalEnd:end,rate:1 }];
  }
  const result: SourceRangeView[] = [];
  for (const {start:from,end:to,entry:{clip}} of visibleTimelineSegments(state)) {
    const active={clipId:clip.id,rate:clip.playbackRate};
    const sourceFrom=clip.sourceIn+(from-clip.startTime)*clip.playbackRate;
    const low=Math.max(start,sourceFrom), high=Math.min(end,sourceFrom+(to-from)*active.rate);
    if (high <= low) continue;
    result.push({ occurrenceId:`${active.clipId}:${from}`,startTime:from+(low-sourceFrom)/active.rate,
      endTime:from+(high-sourceFrom)/active.rate,sourceStart:low,sourceEnd:high,
      originalStart:start,originalEnd:end,rate:active.rate });
  }
  return result;
}

/** Convert a pointer edit on an occurrence back into the stored source range. */
export function editSourceRange(view: SourceRangeView, deltaMs: number, type: 'move'|'start'|'end', sourceDuration: number) {
  const delta=deltaMs*view.rate;
  const length=view.originalEnd-view.originalStart;
  // The native zoom validator excludes ranges of 500ms or less.
  const minimum=Math.min(length,501);
  if (type==='move') {
    const start=Math.max(0,Math.min(sourceDuration-length,view.originalStart+delta));
    return {start_time:Math.round(start),end_time:Math.round(start+length)};
  }
  return type==='start'
    ? {start_time:Math.round(Math.max(0,Math.min(view.originalEnd-minimum,view.sourceStart+delta))),end_time:Math.round(view.originalEnd)}
    : {start_time:Math.round(view.originalStart),end_time:Math.round(Math.min(sourceDuration,Math.max(view.originalStart+minimum,view.sourceEnd+delta)))};
}

export function newSourceZoomRange(state: TimelineState, time: number, playbackLength=2000) {
  const sample=previewSample(state,time);
  if (sample.sourceTime===null || time>=timelineDuration(state)) return null;
  const start=Math.round(sample.sourceTime);
  const end=Math.min(state.duration,Math.round(start+playbackLength*sample.rate));
  return end-start>500 ? {start_time:start,end_time:end} : null;
}
