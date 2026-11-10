import type { SourceTimeResolver } from './sourceTimeResolver';
import type { EditorState, ZoomBlock } from '../../stores/editor/types';
import { MIN_ZOOM_DURATION_MS } from './zoomTiming';
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
  let previousSource: string | undefined;
  for (const {start:from,end:to,entry:{clip}} of visibleTimelineSegments(state)) {
    const active={clipId:clip.id,rate:clip.playbackRate};
    const sourceFrom=clip.sourceIn+(from-clip.startTime)*clip.playbackRate;
    const low=Math.max(start,sourceFrom), high=Math.min(end,sourceFrom+(to-from)*active.rate);
    if (high <= low) continue;
    const view = { occurrenceId:`${active.clipId}:${from}`,startTime:from+(low-sourceFrom)/active.rate,
      endTime:from+(high-sourceFrom)/active.rate,sourceStart:low,sourceEnd:high,
      originalStart:start,originalEnd:end,rate:active.rate };
    const previous = result[result.length - 1];
    // An unchanged source span stays one occurrence across a clip split.
    if (previous && previousSource === clip.sourceFilePath && previous.rate === view.rate
      && Math.abs(previous.endTime - view.startTime) < 1e-7
      && Math.abs(previous.sourceEnd - view.sourceStart) < 1e-7) {
      previous.endTime = view.endTime;
      previous.sourceEnd = view.sourceEnd;
    } else result.push(view);
    previousSource = clip.sourceFilePath;
  }
  return result;
}

export function zoomRangeLimits(blockId: string, blocks: readonly ZoomBlock[], sourceDuration: number) {
  const ordered = [...blocks].sort((a,b) => a.start_time-b.start_time);
  const index = ordered.findIndex(block => block.id === blockId);
  return { start: ordered[index-1]?.end_time ?? 0,
    end: ordered[index+1]?.start_time ?? sourceDuration };
}

export function indexZoomRangeLimits(blocks: readonly ZoomBlock[], sourceDuration: number) {
  const ordered = [...blocks].sort((a,b) => a.start_time-b.start_time);
  return new Map(ordered.map((block, index) => [block.id, {
    start: ordered[index-1]?.end_time ?? 0,
    end: ordered[index+1]?.start_time ?? sourceDuration,
  }]));
}

export function zoomResizeAvailability(view: SourceRangeView, limits: {start:number;end:number}, resolver?: SourceTimeResolver | null) {
  const duration = resolver
    ? resolver.toPlayback(view.originalEnd) - resolver.toPlayback(view.originalStart)
    : (view.originalEnd - view.originalStart) / view.rate;
  const canShrink = duration > MIN_ZOOM_DURATION_MS;
  return {
    start: Math.abs(view.sourceStart - view.originalStart) < 1e-7
      && (view.originalStart > limits.start || canShrink),
    end: Math.abs(view.sourceEnd - view.originalEnd) < 1e-7
      && (view.originalEnd < limits.end || canShrink),
  };
}

/** Convert a pointer edit on an occurrence back into the stored source range. */
export function editSourceRange(view: SourceRangeView, deltaMs: number, type: 'move'|'start'|'end', sourceDuration: number, limits = {start:0,end:sourceDuration}, resolver?: SourceTimeResolver | null) {
  if (resolver) {
    const originalStart=resolver.toPlayback(view.originalStart);
    const originalEnd=resolver.toPlayback(view.originalEnd);
    const lower=resolver.toPlayback(limits.start), upper=resolver.toPlayback(limits.end);
    const length=originalEnd-originalStart;
    const minimum=Math.min(length,MIN_ZOOM_DURATION_MS);
    let start=originalStart, end=originalEnd;
    if (type==='move') {
      if (deltaMs===0 || (start+deltaMs<lower && start<=lower) || (end+deltaMs>upper && end>=upper))
        return {start_time:view.originalStart,end_time:view.originalEnd};
      start=Math.max(lower,Math.min(upper-length,start+deltaMs));
      end=start+length;
    } else if (type==='start') {
      start=Math.max(lower,Math.min(end-minimum,view.startTime+deltaMs));
    } else {
      end=Math.min(upper,Math.max(start+minimum,view.endTime+deltaMs));
    }
    return {start_time:type==='end' ? view.originalStart : Math.round(resolver.toSource(start)),
      end_time:type==='start' ? view.originalEnd : Math.round(resolver.toSource(end))};
  }
  const delta=deltaMs*view.rate;
  const length=view.originalEnd-view.originalStart;
  const minimum=Math.min(length,MIN_ZOOM_DURATION_MS*view.rate);
  if (type==='move') {
    if ((view.originalStart+delta < limits.start && view.originalStart <= limits.start)
      || (view.originalEnd+delta > limits.end && view.originalEnd >= limits.end))
      return {start_time:view.originalStart,end_time:view.originalEnd};
    const start=Math.max(limits.start,Math.min(limits.end-length,view.originalStart+delta));
    return {start_time:Math.round(start),end_time:Math.round(start+length)};
  }
  return type==='start'
    ? {start_time:Math.round(Math.max(limits.start,Math.min(view.originalEnd-minimum,view.sourceStart+delta))),end_time:Math.round(view.originalEnd)}
    : {start_time:Math.round(view.originalStart),end_time:Math.round(Math.min(limits.end,Math.max(view.originalStart+minimum,view.sourceEnd+delta)))};
}

export function newSourceZoomRange(
  state: TimelineState & Partial<Pick<EditorState, 'zoomAnalysis'>>,
  time: number,
  sourceLength = 3000,
): { start_time: number; end_time: number; splitBlockId?: string } | null {
  const sample=previewSample(state,time);
  if (sample.sourceTime===null || time>=timelineDuration(state)) return null;
  const start=Math.round(sample.sourceTime);
  const blocks=state.zoomAnalysis?.zoom_blocks ?? [];
  const containing=blocks.find(block => block.start_time < start && block.end_time > start);
  if (containing) {
    if (start-containing.start_time < MIN_ZOOM_DURATION_MS
      || containing.end_time-start < MIN_ZOOM_DURATION_MS) return null;
    return {start_time:start,end_time:containing.end_time,splitBlockId:containing.id};
  }
  const nextStart=blocks.reduce((next,block) => block.start_time >= start
    ? Math.min(next,block.start_time) : next,state.duration);
  const end=Math.min(nextStart,start+sourceLength);
  return end-start>=MIN_ZOOM_DURATION_MS ? {start_time:start,end_time:end} : null;
}
