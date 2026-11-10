import type { EditorActions, EditorState } from '../../stores/editor/types';
import { previewSample } from './previewTimeline';

export function splitTimelineAtTime(
  state: EditorState & EditorActions,
  time: number,
  zoomId: string | null,
): string[] {
  if (zoomId !== null) {
    const sourceTime = previewSample(state, time).sourceTime;
    if (sourceTime !== null) state.splitZoomBlockAtTime(zoomId, Math.round(sourceTime));
    return [];
  }
  return state.cutClipsAtTime(time);
}
