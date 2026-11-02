import type { EditorActions, EditorState } from '../../../../stores/editor/types';

/** Playback position and selection do not change a precomputed motion path. */
export function selectMotionInputs(state: EditorState & EditorActions) {
  const output = state.getExportDimensions();
  return {
    videoFilePath: state.videoFilePath, duration: state.duration,
    clips: state.clips, tracks: state.tracks, trimStart: state.trimStart, trimEnd: state.trimEnd,
    zoomAnalysis: state.zoomAnalysis, visualSettings: state.visualSettings,
    videoWidth: state.videoWidth, videoHeight: state.videoHeight,
    captureMode: state.captureMode, screenResolution: state.screenResolution,
    recordingArea: state.recordingArea, webcamKeyframes: state.webcamKeyframes,
    outputWidth: output.width, outputHeight: output.height,
  };
}
