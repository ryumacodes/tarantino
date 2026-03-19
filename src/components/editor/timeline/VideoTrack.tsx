import React from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Film, Eye } from 'lucide-react';

interface Clip {
  id: string;
  trackId: string;
  startTime: number;
  duration: number;
}

interface VideoTrackProps {
  timelineWidth: number;
  pixelsPerMs: number;
  thumbnails: string[];
  thumbnailsLoading: boolean;
  trimStart: number;
  trimEnd: number;
  duration: number;
  clips: Clip[];
  isExporting: boolean;
  onToggleVisibility: () => void;
  onTrimStartDrag: (e: React.MouseEvent) => void;
  onTrimEndDrag: (e: React.MouseEvent) => void;
}

const VideoTrack: React.FC<VideoTrackProps> = ({
  timelineWidth,
  pixelsPerMs,
  thumbnails,
  thumbnailsLoading,
  trimStart,
  trimEnd,
  duration,
  clips,
  isExporting,
  onToggleVisibility,
  onTrimStartDrag,
  onTrimEndDrag,
}) => {
  const videoClips = clips?.filter(c => c.trackId === 'video-track-main') || [];

  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label">
          <Film size={12} />
          <span>Video</span>
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
        {/* Video thumbnails */}
        <div className="video-thumbnails timeline-track--golden" style={{ width: `${timelineWidth}px` }}>
          {thumbnailsLoading ? (
            <div className="thumbnails-loading">
              <div className="editor-spinner" />
            </div>
          ) : (
            thumbnails.map((thumbnailPath, index) => {
              const thumbnailWidth = timelineWidth / thumbnails.length;
              const thumbnailLeft = index * thumbnailWidth;

              return (
                <img
                  key={index}
                  src={convertFileSrc(thumbnailPath)}
                  alt={`Frame ${index}`}
                  className="video-thumbnail"
                  onError={(e) => {
                    console.error(`Failed to load thumbnail ${index}:`, thumbnailPath);
                    e.currentTarget.style.opacity = '0.3';
                  }}
                  style={{
                    left: `${thumbnailLeft}px`,
                    width: `${thumbnailWidth + 1}px`,
                    height: '60px',
                    objectFit: 'cover'
                  }}
                />
              );
            })
          )}
        </div>

        {/* Trim handles */}
        <div
          className={`trim-handle trim-handle--start ${isExporting ? 'disabled' : ''}`}
          style={{
            left: `${trimStart * pixelsPerMs}px`,
            cursor: isExporting ? 'not-allowed' : 'ew-resize',
            opacity: isExporting ? 0.4 : 0.8
          }}
          onMouseDown={onTrimStartDrag}
        />
        <div
          className={`trim-handle trim-handle--end ${isExporting ? 'disabled' : ''}`}
          style={{
            left: `${trimEnd * pixelsPerMs}px`,
            cursor: isExporting ? 'not-allowed' : 'ew-resize',
            opacity: isExporting ? 0.4 : 0.8
          }}
          onMouseDown={onTrimEndDrag}
        />

        {/* Trimmed overlays */}
        <div
          className="trim-overlay trim-overlay--start"
          style={{ width: `${trimStart * pixelsPerMs}px` }}
        />
        <div
          className="trim-overlay trim-overlay--end"
          style={{
            left: `${trimEnd * pixelsPerMs}px`,
            width: `${(duration - trimEnd) * pixelsPerMs}px`
          }}
        />

        {/* Clip boundaries */}
        {videoClips.length > 1 && (
          videoClips.map((clip, index) => {
            if (index === 0) return null;
            return (
              <div
                key={`clip-boundary-${clip.id}`}
                className="clip-boundary"
                style={{
                  left: `${clip.startTime * pixelsPerMs}px`,
                  height: '100%',
                  position: 'absolute',
                  top: 0,
                  width: '2px',
                  background: 'linear-gradient(to bottom, #f59e0b 0%, #f59e0b 40%, transparent 40%, transparent 60%, #f59e0b 60%, #f59e0b 100%)',
                  zIndex: 10,
                  pointerEvents: 'none',
                }}
                title={`Cut at ${(clip.startTime / 1000).toFixed(2)}s`}
              />
            );
          })
        )}
      </div>
    </div>
  );
};

export default VideoTrack;
