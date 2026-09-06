import CaptureSettings, { type CaptureConfig } from './CaptureSettings';
import VideoOutputLocation from './VideoOutputLocation';
import PermissionStatus from './PermissionStatus';
import { AlertTriangle } from 'lucide-react';
import type { RecordingCapabilities } from '../utils/recordingCapabilities';

interface CaptureSettingsPanelProps {
  recordingCapabilities: RecordingCapabilities | null;
  checkingRecordingSupport: boolean;
  checkRecordingSupport: () => Promise<void>;
  captureConfig: CaptureConfig;
  setCaptureConfig: (config: CaptureConfig) => void;
  selectedDisplay: { width: number; height: number } | null;
}

export default function CaptureSettingsPanel({
  recordingCapabilities, checkingRecordingSupport, checkRecordingSupport,
  captureConfig, setCaptureConfig, selectedDisplay,
}: CaptureSettingsPanelProps) {
  return (
    <div className="capture-bar__dropdown capture-bar__dropdown--settings">
      <div className="space-y-4">
        <VideoOutputLocation />
        {recordingCapabilities?.unavailableReason && (
          <div className="capture-bar__runtime-detail" role="alert">
            <AlertTriangle size={16} />
            <div>
              <strong>Recording unavailable</strong>
              <span>{recordingCapabilities.unavailableReason}</span>
              <button
                className="capture-bar__runtime-retry"
                disabled={checkingRecordingSupport}
                onClick={checkRecordingSupport}
              >
                {checkingRecordingSupport ? 'Checking…' : 'Check again'}
              </button>
            </div>
          </div>
        )}

        {recordingCapabilities?.usesSystemSourcePicker ? (
          <div className="capture-bar__portal-notice">
            Display capture defaults to the display containing Tarantino. For Window capture,
            choose the window option first; Record then opens your desktop's secure window picker.
            {!recordingCapabilities.automaticZoomAvailable && (
              <span className="capture-bar__portal-limit">
                Automatic click zoom needs permission to read a compatible mouse device. Recording
                and manual zoom editing remain available.
              </span>
            )}
          </div>
        ) : <PermissionStatus className="mb-4" />}

        <CaptureSettings
          compact
          config={captureConfig}
          onChange={setCaptureConfig}
          sourceResolution={selectedDisplay ? {
            width: selectedDisplay.width,
            height: selectedDisplay.height
          } : undefined}
        />
      </div>
    </div>
  );
}
