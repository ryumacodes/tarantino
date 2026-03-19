import React from 'react';
import { Film, FastForward } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import type { TabProps } from './types';

const speedPresets = [
  { rate: 0.5, label: '0.5x', description: 'Slow motion' },
  { rate: 1.0, label: '1x', description: 'Normal' },
  { rate: 1.5, label: '1.5x', description: 'Faster' },
  { rate: 2.0, label: '2x', description: 'Fast' },
  { rate: 4.0, label: '4x', description: 'Very fast' },
];

const formatTime = (ms: number): string => {
  const seconds = Math.floor(ms / 1000);
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const ClipsTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { tracks, setClipPlaybackRate } = useEditorStore();

  // Get all clips from all tracks
  const allClips = tracks.flatMap((track) => track.clips);

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Clip Speed Control</h3>
          <p>Speed up or slow down video segments</p>
        </div>

        {allClips.length === 0 ? (
          <div className="empty-state">
            <Film size={24} />
            <p>No clips yet</p>
            <span>Use the scissors tool to cut your video into clips</span>
          </div>
        ) : (
          <div className="clips-list">
            {allClips.map((clip, index) => (
              <div key={clip.id} className="clip-item">
                <div className="clip-header">
                  <div className="clip-info">
                    <div className="clip-name">
                      {clip.name || `Clip ${index + 1}`}
                    </div>
                    <div className="clip-details">
                      {formatTime(clip.startTime)} - {formatTime(clip.startTime + clip.duration)} • {(clip.duration / 1000).toFixed(1)}s
                    </div>
                  </div>
                  <div className="clip-speed-badge">
                    <FastForward size={12} />
                    {clip.playbackRate}x
                  </div>
                </div>

                <div className="speed-presets">
                  {speedPresets.map(({ rate, label, description }) => (
                    <button
                      key={rate}
                      className={`speed-preset-btn ${clip.playbackRate === rate ? 'active' : ''}`}
                      onClick={() => setClipPlaybackRate(clip.id, rate)}
                      title={description}
                      disabled={isExporting}
                    >
                      {label}
                    </button>
                  ))}
                </div>

                <div className="control-group">
                  <label>Custom Speed</label>
                  <div className="slider-control">
                    <input
                      type="range"
                      min="0.25"
                      max="4"
                      step="0.25"
                      value={clip.playbackRate}
                      onChange={(e) => setClipPlaybackRate(clip.id, parseFloat(e.target.value))}
                      className="editor-slider"
                      disabled={isExporting}
                    />
                    <span className="value-display">{clip.playbackRate}x</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Tips</h3>
        </div>
        <div className="tips-content">
          <p>• Use scissors tool (S key) to cut clips at playhead</p>
          <p>• Speed up boring sections with 2x-4x</p>
          <p>• Slow down important actions with 0.5x</p>
        </div>
      </div>
    </div>
  );
};

export default ClipsTab;
