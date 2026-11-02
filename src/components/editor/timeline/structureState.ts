import type { EditorActions, EditorState } from '../../../stores/editor/types';

/** Playback ticks belong to the small clock views, not the track/thumbnail tree. */
export function timelineStructureState(state: EditorState & EditorActions) {
  const {currentTime, isPlaying, playbackRevision, ...structure} = state;
  return structure;
}
