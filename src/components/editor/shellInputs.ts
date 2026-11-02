import type { EditorActions, EditorState } from '../../stores/editor/types';

/** The editor shell owns playback scheduling, not the per-frame time display. */
export function selectEditorShellInputs(state: EditorState & EditorActions) {
  return {
    isPlaying: state.isPlaying, setIsPlaying: state.setIsPlaying,
    setCurrentTime: state.setCurrentTime, videoFilePath: state.videoFilePath,
    projectTitle: state.projectTitle, recordedAt: state.recordedAt,
    setProjectTitle: state.setProjectTitle,
  };
}
