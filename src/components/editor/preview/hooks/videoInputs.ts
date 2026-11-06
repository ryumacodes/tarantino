import { previewSample } from '../../../../utils/timeline/previewTimeline';
import type { EditorActions, EditorState } from '../../../../stores/editor/types';

export function selectVideoInputs(state: EditorState & EditorActions) {
  const output = state.getExportDimensions();
  return {
    videoFilePath: state.videoFilePath, loadMouseEvents: state.loadMouseEvents,
    visualSettings: state.visualSettings, displayResolution: state.displayResolution,
    screenResolution: state.screenResolution, recordingArea: state.recordingArea,
    videoWidth: state.videoWidth, videoHeight: state.videoHeight, captureMode: state.captureMode,
    outputWidth: output.width, outputHeight: output.height,
  };
}

export function selectPreviewInputs(state: EditorState & EditorActions) {
  const output = state.getExportDimensions();
  return {
    visualSettings: state.visualSettings, videoFilePath: state.videoFilePath,
    displayResolution: state.displayResolution, hasWebcam: state.hasWebcam,
    outputWidth: output.width, outputHeight: output.height,
  };
}

export function selectPlaybackSecond(state: Pick<EditorState, 'currentTime'>) {
  return Math.floor(state.currentTime / 1000);
}

export function selectVideoMaterialInputs(state: EditorState & EditorActions) {
  return {
    setCurrentTime: state.setCurrentTime, setDuration: state.setDuration,
    duration: state.duration, videoFilePath: state.videoFilePath,
    hasMicrophone: state.hasMicrophone, hasSystemAudio: state.hasSystemAudio,
    audioSettings: state.audioSettings,
    gap: previewSample(state, state.currentTime).sourceTime === null,
  };
}
