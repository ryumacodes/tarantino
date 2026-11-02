import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';

export type MergeDirection = 'previous' | 'next';
export function mergeNeighbor(clips: TimelineClip[], tracks: TimelineTrack[], id: string, direction: MergeDirection) {
  const selected = clips.find(clip => clip.id === id);
  if (!selected || selected.locked || tracks.find(track => track.id === selected.trackId)?.locked) return null;
  const ordered = clips.filter(clip => clip.trackId === selected.trackId).sort((a,b) => a.startTime-b.startTime);
  const index = ordered.findIndex(clip => clip.id === id);
  const neighbor = ordered[index + (direction === 'next' ? 1 : -1)];
  if (!neighbor || neighbor.locked || selected.type !== neighbor.type
    || selected.sourceFilePath !== neighbor.sourceFilePath) return null;
  const [first,last] = direction === 'next' ? [selected,neighbor] : [neighbor,selected];
  // A single source range cannot represent reordered or overlapping media.
  if (first.sourceOut > last.sourceIn || first.startTime+first.duration > last.startTime+1e-6) return null;
  if (![first,last].every(clip => Number.isFinite(clip.playbackRate) && clip.playbackRate > 0
    && clip.duration > 0 && clip.sourceOut > clip.sourceIn)) return null;
  return { selected, neighbor, first, last };
}
