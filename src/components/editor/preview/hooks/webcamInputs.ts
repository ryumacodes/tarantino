import type { EditorActions, EditorState } from '../../../../stores/editor/types';
import type { SilenceRange } from '../../../../utils/camera/silence';
import { sampleWebcam } from '../../../../utils/camera/webcamTimeline';
import { previewSample } from '../../../../utils/timeline/previewTimeline';

export function selectWebcamInputs(state: EditorState & EditorActions, silence: readonly SilenceRange[]) {
  const source = previewSample(state, state.currentTime);
  return {
    ...sampleWebcam(state.visualSettings, state.webcamKeyframes, source.sourceTime ?? 0, silence),
    gap: source.sourceTime === null,
    mirrored: state.visualSettings.webcamMirrored,
    updateVisualSettings: state.updateVisualSettings,
    updateWebcamKeyframe: state.updateWebcamKeyframe,
  };
}
