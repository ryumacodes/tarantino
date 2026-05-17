import React from 'react';
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
  Maximize2,
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Monitor
} from 'lucide-react';

interface ZoomControlsProps {
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomReset: () => void;
}

export const ZoomControls: React.FC<ZoomControlsProps> = ({
  zoom,
  onZoomIn,
  onZoomOut,
  onZoomReset
}) => (
  <div className="zoom-controls">
    <button
      className="editor-btn editor-btn--ghost editor-btn--small"
      onClick={onZoomOut}
      title="Zoom Out"
    >
      <ZoomOut size={14} />
    </button>
    <span className="zoom-level">{Math.round(zoom * 100)}%</span>
    <button
      className="editor-btn editor-btn--ghost editor-btn--small"
      onClick={onZoomIn}
      title="Zoom In"
    >
      <ZoomIn size={14} />
    </button>
    <button
      className="editor-btn editor-btn--ghost editor-btn--small"
      onClick={onZoomReset}
      title="Reset Zoom"
    >
      <RotateCcw size={14} />
    </button>
  </div>
);

interface ViewControlsProps {
  isFullscreen: boolean;
  onFullscreen: () => void;
}

export const ViewControls: React.FC<ViewControlsProps> = ({
  isFullscreen,
  onFullscreen
}) => (
  <div className="view-controls">
    <button
      className="editor-btn editor-btn--ghost editor-btn--small"
      onClick={onFullscreen}
      title={isFullscreen ? "Exit Fullscreen" : "Fullscreen"}
    >
      {isFullscreen ? <Monitor size={14} /> : <Maximize2 size={14} />}
    </button>
  </div>
);

interface PlaybackControlsProps {
  isPlaying: boolean;
  isMuted: boolean;
  currentTime: number;
  duration: number;
  onPlayPause: () => void;
  onSeekBackward: () => void;
  onSeekForward: () => void;
  onMuteToggle: () => void;
}

const formatTime = (ms: number): string => {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
};

export const PlaybackControls: React.FC<PlaybackControlsProps> = ({
  isPlaying,
  isMuted,
  currentTime,
  duration,
  onPlayPause,
  onSeekBackward,
  onSeekForward,
  onMuteToggle
}) => (
  <div className="playback-controls">
    <div className="playback-buttons">
      <button
        className="editor-btn editor-btn--ghost editor-btn--icon"
        onClick={onSeekBackward}
        title="Go to Start"
      >
        <SkipBack size={18} />
      </button>

      <button
        className="play-pause-button"
        onClick={onPlayPause}
        title={isPlaying ? "Pause" : "Play"}
      >
        {isPlaying ? <Pause size={20} /> : <Play size={20} />}
      </button>

      <button
        className="editor-btn editor-btn--ghost editor-btn--icon"
        onClick={onSeekForward}
        title="Go to End"
      >
        <SkipForward size={18} />
      </button>
    </div>

    <div className="time-display">
      <span className="current-time">{formatTime(currentTime)}</span>
      <span className="time-separator">/</span>
      <span className="total-time">{formatTime(duration)}</span>
    </div>

    <div className="audio-controls">
      <button
        className="editor-btn editor-btn--ghost editor-btn--icon"
        onClick={onMuteToggle}
        title={isMuted ? "Unmute" : "Mute"}
      >
        {isMuted ? <VolumeX size={16} /> : <Volume2 size={16} />}
      </button>
    </div>
  </div>
);
