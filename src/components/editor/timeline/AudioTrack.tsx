import React from 'react';
import { Mic, Volume2, Eye } from 'lucide-react';
import AudioWaveform from '../AudioWaveform';

interface AudioTrackProps {
  type: 'microphone' | 'system';
  duration: number;
  pixelsPerMs: number;
  audioPath: string | null;
  isExporting: boolean;
  onToggleVisibility: () => void;
}

const AudioTrack: React.FC<AudioTrackProps> = ({
  type,
  duration,
  pixelsPerMs,
  audioPath,
  isExporting,
  onToggleVisibility,
}) => {
  const Icon = type === 'microphone' ? Mic : Volume2;
  const label = type === 'microphone' ? 'Microphone' : 'Audio';

  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label">
          <Icon size={12} />
          <span>{label}</span>
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
        <AudioWaveform
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
