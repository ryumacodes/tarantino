import React from 'react';
import { Camera, Eye } from 'lucide-react';
import type { WebcamKeyframe } from '../../../stores/editor/types';

interface WebcamTrackProps {
  pixelsPerMs: number;
  webcamKeyframes: WebcamKeyframe[];
  isExporting: boolean;
  onToggleVisibility: () => void;
}

const WebcamTrack: React.FC<WebcamTrackProps> = ({
  pixelsPerMs,
  webcamKeyframes,
  isExporting,
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
          onClick={onToggleVisibility}
          disabled={isExporting}
        >
          <Eye size={12} />
        </button>
      </div>
      <div className="track-content">
        {webcamKeyframes.map(kf => (
          <div
            key={kf.id}
            className={`webcam-keyframe ${!kf.visible ? 'webcam-keyframe--hidden' : ''}`}
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
