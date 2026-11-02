import React from 'react';
import { Camera, Eye, EyeOff } from 'lucide-react';
import type { WebcamKeyframe } from '../../../stores/editor/types';

interface WebcamTrackProps {
  pixelsPerMs: number;
  webcamKeyframes: WebcamKeyframe[];
  isExporting: boolean;
  enabled: boolean;
  onToggleVisibility: () => void;
}

const WebcamTrack: React.FC<WebcamTrackProps> = ({
  pixelsPerMs,
  webcamKeyframes,
  isExporting,
  enabled,
  onToggleVisibility,
}) => {
  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label">
          <Camera size={12} />
          <span>Webcam</span>
        </div>
        <button
          className={`track-toggle ${isExporting ? 'disabled' : ''}`}
          aria-label="Enable camera"
          aria-pressed={enabled}
          title={enabled ? 'Hide camera' : 'Show camera'}
          onClick={event => { event.stopPropagation(); onToggleVisibility(); }}
          disabled={isExporting}
        >
          {enabled ? <Eye size={12} /> : <EyeOff size={12} />}
        </button>
      </div>
      <div className="track-content" style={{ opacity: enabled ? 1 : 0.45 }}>
        {webcamKeyframes.map(kf => (
          <div
            key={kf.id}
            title={`${kf.visible ? kf.layout ?? 'overlay' : 'hidden'} camera at ${(kf.time / 1000).toFixed(2)}s`}
            className={`webcam-keyframe ${!kf.visible || kf.layout === 'hidden' ? 'webcam-keyframe--hidden' : ''}`}
            style={{
              left: `${kf.time * pixelsPerMs}px`,
              width: '3px'
            }}
          />
        ))}
      </div>
    </div>
  );
};

export default WebcamTrack;
