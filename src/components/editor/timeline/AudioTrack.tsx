import React from 'react';
import { Mic, Volume2, VolumeX } from 'lucide-react';
import AudioWaveform from '../AudioWaveform';

interface AudioTrackProps {
  type: 'microphone' | 'system';
  duration: number;
  viewportLeft:number;
  viewportRight:number;
  pixelsPerMs: number;
  audioPath: string | null;
  isExporting: boolean;
  muted: boolean;
  onToggleMute: () => void;
}

const AudioTrack: React.FC<AudioTrackProps> = ({
  type,
  duration,
  viewportLeft,viewportRight,
  pixelsPerMs,
  audioPath,
  isExporting,
  muted,
  onToggleMute,
}) => {
  const Icon = type === 'microphone' ? Mic : Volume2;
  const label = type === 'microphone' ? 'Microphone' : 'System audio';

  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label">
          <Icon size={12} />
          <span>{label}</span>
        </div>
        <button
          className={`track-toggle ${isExporting ? 'disabled' : ''}`}
          aria-label={`Mute ${label.toLowerCase()}`}
          aria-pressed={muted}
          title={`${muted ? 'Unmute' : 'Mute'} ${label.toLowerCase()}`}
          onClick={event => { event.stopPropagation(); onToggleMute(); }}
          disabled={isExporting}
        >
          {muted ? <VolumeX size={12} /> : <Volume2 size={12} />}
        </button>
      </div>
      <div className="track-content" style={{ opacity: muted ? 0.45 : 1 }}>
        <AudioWaveform
          viewportLeft={viewportLeft} viewportRight={viewportRight}
          type={type}
          duration={duration}
          pixelsPerMs={pixelsPerMs}
          audioPath={audioPath}
        />
      </div>
    </div>
  );
};

export default AudioTrack;
