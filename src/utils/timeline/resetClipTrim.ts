import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';

export type TrimSide = 'start' | 'end';

export function restoredClipBounds(clips: TimelineClip[], tracks: TimelineTrack[], id: string,
  sourceDuration: number, sourcePath: string | null, side?: TrimSide) {
  const clip = clips.find(item => item.id === id);
  if (!clip || clip.locked || tracks.find(track => track.id === clip.trackId)?.locked
    || !['video', 'screen'].includes(clip.type)
    || (clip.sourceFilePath && clip.sourceFilePath !== sourcePath)
    || !Number.isFinite(sourceDuration) || sourceDuration <= 0
    || !Number.isFinite(clip.playbackRate) || clip.playbackRate <= 0) return null;
  const ordered = clips.filter(item => item.trackId === clip.trackId).sort((a,b) => a.startTime-b.startTime);
  const index = ordered.findIndex(item => item.id === id);
  const previous = ordered[index-1], next = ordered[index+1];
  const sameSource = (item: TimelineClip) => (item.sourceFilePath || sourcePath) === (clip.sourceFilePath || sourcePath);
  // A neighboring clip from another source cannot define a source trim boundary.
  const start = side === 'end' ? clip.sourceIn : previous
    ? sameSource(previous) ? previous.sourceOut : clip.sourceIn : 0;
  const end = side === 'start' ? clip.sourceOut : next
    ? sameSource(next) ? next.sourceIn : clip.sourceOut : sourceDuration;
  if (start < 0 || start > clip.sourceIn || end < clip.sourceOut || end > sourceDuration
    || end <= start || (start === clip.sourceIn && end === clip.sourceOut)) return null;
  return { clip, start, end };
}
