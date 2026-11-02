import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
export interface TimelineState {
  clips: TimelineClip[]; tracks: TimelineTrack[]; duration: number;
}
export function visibleVideoClips(state: TimelineState) {
  const tracks = state.tracks.filter(t => t.type === 'video' || t.type === 'screen');
  const solo = tracks.some(t => t.solo);
  return state.clips.flatMap(clip => {
    const track = tracks.find(t => t.id === clip.trackId);
    if (!track || !track.visible || !clip.enabled || (solo && !track.solo)) return [];
    return [{ clip, track }];
  });
}
interface TimelineSegment {
  start: number; end: number;
  entry: ReturnType<typeof visibleVideoClips>[number];
}
let cachedClips: TimelineClip[] | undefined;
let cachedTracks: TimelineTrack[] | undefined;
let cachedIndex = { segments: [] as TimelineSegment[], duration: 0, legacy: true };

// Build the visible intervals only when clips/track policy change. Heap priority
// is track order followed by original clip order, matching stable layer selection.
function timelineIndex(state: TimelineState) {
  if (state.clips === cachedClips && state.tracks === cachedTracks) return cachedIndex;
  const entries = visibleVideoClips(state).sort((a,b) => a.track.order-b.track.order);
  const events = entries.flatMap(({clip},index) => clip.duration > 0 && Number.isFinite(clip.startTime) && Number.isFinite(clip.startTime+clip.duration) ? [
    {time:clip.startTime,index,enter:true},
    {time:clip.startTime+clip.duration,index,enter:false},
  ] : []).sort((a,b) => a.time-b.time);
  const active = entries.map(() => false);
  const heap: number[] = [];
  const push = (value: number) => {
    let i=heap.length; heap.push(value);
    while(i>0) {
      const parent=(i-1)>>1;
      if(heap[parent]<=value) break;
      heap[i]=heap[parent]; i=parent;
    }
    heap[i]=value;
  };
  const pop = () => {
    const value=heap.pop()!;
    if(!heap.length) return;
    let i=0;
    while(i*2+1<heap.length) {
      let child=i*2+1;
      if(child+1<heap.length && heap[child+1]<heap[child]) child++;
      if(heap[child]>=value) break;
      heap[i]=heap[child]; i=child;
    }
    heap[i]=value;
  };
  const segments: TimelineSegment[] = [];
  for(let i=0;i<events.length;) {
    const time=events[i].time;
    while(i<events.length && events[i].time===time) {
      const event=events[i++]; active[event.index]=event.enter;
      if(event.enter) push(event.index);
    }
    while(heap.length && !active[heap[0]]) pop();
    if(heap.length && i<events.length) segments.push({start:time,end:events[i].time,entry:entries[heap[0]]});
  }
  cachedClips=state.clips; cachedTracks=state.tracks;
  cachedIndex={segments,legacy:!state.tracks.some(t => t.type==='video'||t.type==='screen'),
    duration:state.clips.reduce((end,c) => c.type==='video'||c.type==='screen' ? Math.max(end,c.startTime+c.duration) : end,0)};
  return cachedIndex;
}
export function timelineDuration(state: TimelineState): number {
  const index=timelineIndex(state);
  return index.legacy ? state.duration : index.duration;
}
export function visibleTimelineSegments(state: TimelineState) {
  return timelineIndex(state).segments;
}
export function previewSample(state: TimelineState, timeMs: number) {
  const index=timelineIndex(state);
  const duration=index.legacy ? state.duration : index.duration;
  // A stopped playhead at the project endpoint holds its final decoded frame.
  if (timeMs > 0 && timeMs === duration) timeMs -= 0.001;
  let low=0, high=index.segments.length;
  while(low<high) {
    const middle=(low+high)>>1;
    if(index.segments[middle].start<=timeMs) low=middle+1; else high=middle;
  }
  const segment=index.segments[low-1];
  const active=segment && timeMs<segment.end ? segment.entry : undefined;
  if (active) {
    const { clip, track } = active;
    return { sourceTime: clip.sourceIn + (timeMs - clip.startTime) * clip.playbackRate,
      micVolume: clip.micVolume ?? 1, systemVolume: clip.systemVolume ?? 1,
      hideCursor: clip.hideCursor ?? false, rate: clip.playbackRate, muted: track.muted, clipId: clip.id, sourceFilePath: clip.sourceFilePath };
  }
  const legacy = index.legacy;
  return { micVolume: 1, systemVolume: 1, hideCursor: !legacy, sourceTime: legacy ? timeMs : null, rate: 1, muted: !legacy, clipId: null, sourceFilePath: undefined };
}
