import { previewSample, type TimelineState } from './previewTimeline';

/** The preceding clip on the currently displayed recording track. */
export function removablePreviousClip(state: TimelineState, time: number) {
  const activeId = previewSample(state,time).clipId;
  const active = state.clips.find(clip => clip.id === activeId);
  if (!active || state.tracks.find(track => track.id === active.trackId)?.locked) return null;
  const ordered = state.clips.filter(clip => clip.trackId === active.trackId).sort((a,b) => a.startTime-b.startTime);
  const previous = ordered[ordered.findIndex(clip => clip.id === active.id)-1];
  return previous && !previous.locked ? previous : null;
}
