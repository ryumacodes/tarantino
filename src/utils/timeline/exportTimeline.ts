import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { timelineDuration, visibleVideoClips } from './previewTimeline';

/** Canonical clips drive export; track copies only supply visibility/order/audio policy. */
export function exportTimeline(state: {
  clips: TimelineClip[]; tracks: TimelineTrack[];
  duration: number; trimStart: number; trimEnd: number;
}) {
  const videoTracks = state.tracks.filter(t => t.type === 'video' || t.type === 'screen');
  if (!videoTracks.length) return {};
  const extent = timelineDuration(state);
  const clips = visibleVideoClips(state).map(({ clip: c, track }) => ({
    id: c.id, startTime: c.startTime, duration: c.duration,
    sourceIn: c.sourceIn, sourceOut: c.sourceOut, playbackRate: c.playbackRate,
    micVolume: c.micVolume ?? 1, systemVolume: c.systemVolume ?? 1,
    hideCursor: c.hideCursor ?? false, disableCursorSmoothing: c.disableCursorSmoothing ?? false, sourceFilePath: c.sourceFilePath, trackOrder: track.order, muted: track.muted,
  }));
  // Speed edits remap the stored trim bounds into edited time.
  const end = Math.min(state.trimEnd, extent);
  if (end <= state.trimStart) throw new Error('Select a nonempty video range before exporting.');
  // Native trim bounds use whole milliseconds. Round outward so a fractional
  // selection survives transport without dropping either selected endpoint.
  return { timeline_clips: clips, trim_start: Math.floor(state.trimStart), trim_end: Math.ceil(end) };
}
