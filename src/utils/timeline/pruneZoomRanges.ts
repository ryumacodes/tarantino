import type { EditorState } from '../../stores/editor/types';

/** Zoom timestamps belong to the original recording, not the edited clock. */
export function pruneZoomRanges(state: EditorState) {
  const clips = state.clips.filter(clip => (clip.type === 'video' || clip.type === 'screen')
    && (!clip.sourceFilePath || clip.sourceFilePath === state.videoFilePath));
  // Visibility and solo are reversible viewing choices, not deleted source.
  const present = (time: number) => clips.some(clip => time >= clip.sourceIn && time <= clip.sourceOut);
  const retained = (start: number, end: number) => present(start) || present(end);
  if (state.zoomAnalysis) {
    state.zoomAnalysis.zoom_blocks = state.zoomAnalysis.zoom_blocks.filter(block => retained(block.start_time, block.end_time));
    if (state.selectedBlockId && !state.zoomAnalysis.zoom_blocks.some(block => block.id === state.selectedBlockId)) {
      state.selectedBlockId = null;
    }
  }
  state.zoomKeyframes = state.zoomKeyframes.filter(frame => retained(frame.time, frame.time+frame.duration));
  if (state.selectedKeyframe && !state.zoomKeyframes.some(frame => frame.id === state.selectedKeyframe?.id)) {
    state.selectedKeyframe = null;
  }
}
