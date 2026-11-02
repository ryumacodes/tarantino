import type { EditorState, TimelineClip } from '../../stores/editor/types';
import type { TrimSide } from './resetClipTrim';

export function trimClipEdge(state: EditorState, id: string, side: TrimSide, sourceTime: number) {
  const clip = state.clips.find(item => item.id === id);
  if (!clip || clip.locked || !Number.isFinite(sourceTime) || !['video','screen'].includes(clip.type)
    || state.tracks.find(track => track.id === clip.trackId)?.locked
    || (clip.sourceFilePath && clip.sourceFilePath !== state.videoFilePath)
    || !Number.isFinite(clip.playbackRate) || clip.playbackRate <= 0) return;
  const ordered = state.clips.filter(item => item.trackId === clip.trackId).sort((a,b) => a.startTime-b.startTime);
  const index = ordered.findIndex(item => item.id === id);
  const previous = ordered[index-1], next = ordered[index+1];
  const sameSource = (item: TimelineClip) => (item.sourceFilePath || state.videoFilePath) === (clip.sourceFilePath || state.videoFilePath);
  const lower = previous ? sameSource(previous) ? previous.sourceOut : clip.sourceIn : 0;
  const upper = next ? sameSource(next) ? next.sourceIn : clip.sourceOut : state.duration;
  if (lower > clip.sourceIn || upper < clip.sourceOut) return;
  // Preserve existing short clips, but do not trim ordinary clips below 100 ms of source.
  const minimum = Math.min(100, clip.sourceOut-clip.sourceIn);
  const start = side === 'start' ? Math.max(lower, Math.min(clip.sourceOut-minimum, sourceTime)) : clip.sourceIn;
  const end = side === 'end' ? Math.min(upper, Math.max(clip.sourceIn+minimum, sourceTime)) : clip.sourceOut;
  if (end <= start || (start === clip.sourceIn && end === clip.sourceOut)) return;
  const oldEnd = clip.startTime+clip.duration;
  const duration = (end-start)/clip.playbackRate;
  const delta = duration-clip.duration;
  const leading = (clip.sourceIn-start)/clip.playbackRate;
  const remap = (time: number) => time < clip.startTime ? time : time >= oldEnd ? time+delta
    : Math.max(clip.startTime, Math.min(clip.startTime+duration, time+leading));
  state.currentTime = remap(state.currentTime);
  state.trimStart = state.trimStart === clip.startTime ? state.trimStart : remap(state.trimStart);
  state.trimEnd = remap(state.trimEnd);
  for (const following of state.clips) {
    if (following.id !== id && following.trackId === clip.trackId && following.startTime >= oldEnd) following.startTime += delta;
  }
  clip.sourceIn=start; clip.sourceOut=end; clip.duration=duration;
  const track = state.tracks.find(track => track.id === clip.trackId);
  if (track) track.clips=state.clips.filter(item => item.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
  if (state.trimEnd <= state.trimStart) {
    state.trimStart = clip.startTime;
    state.trimEnd = clip.startTime+duration;
  }
  state.playbackRevision=state.playbackRevision+1;
}
