import { isLinuxRuntime } from './platform';

export interface RecordingCapabilities {
  shortcutRecordingAvailable?: boolean;
  recordingAvailable: boolean;
  unavailableReason: string | null;
  usesSystemSourcePicker: boolean;
  displayPreviewAvailable: boolean;
  automaticZoomAvailable: boolean;
  systemAudioAvailable: boolean;
}
export const optimisticNativeCapabilities: RecordingCapabilities = {
  recordingAvailable: true,
  unavailableReason: null,
  usesSystemSourcePicker: false,
  displayPreviewAvailable: true,
  automaticZoomAvailable: true,
  systemAudioAvailable: !isLinuxRuntime,
};
