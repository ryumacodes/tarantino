import { TypingSpeedSuggestions } from "./TypingSpeedSuggestions";
import { restoredClipBounds } from '../../../utils/timeline/resetClipTrim';
import { useShallow } from 'zustand/react/shallow';
import { mergeNeighbor } from '../../../utils/timeline/mergeClips';
import React from 'react';
import { Film, FastForward, Trash2 } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import type { TabProps } from './types';

const speedPresets = [0.5, 0.75, 1, 1.2, 1.4, 1.6, 1.8, 2, 3, 4, 8, 16, 24]
  .map(rate => ({ rate, label: `${rate}x`, description: `${rate}× playback speed` }));

const formatTime = (ms: number): string => {
  const seconds = Math.floor(ms / 1000);
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const ClipsTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { tracks, duration, videoFilePath, resetClipTrim, setClipPlaybackRate, applyClipSpeedToTrack, deleteClip, updateClip, mergeClip, hasMicrophone, hasSystemAudio } = useEditorStore(useShallow(state => ({
    tracks: state.tracks,
    duration: state.duration,
    videoFilePath: state.videoFilePath,
    resetClipTrim: state.resetClipTrim,
    setClipPlaybackRate: state.setClipPlaybackRate,
    applyClipSpeedToTrack: state.applyClipSpeedToTrack,
    deleteClip: state.deleteClip,
    updateClip: state.updateClip,
    mergeClip: state.mergeClip,
    hasMicrophone: state.hasMicrophone,
    hasSystemAudio: state.hasSystemAudio,
  })));

  // Get all clips from all tracks
  const allClips = tracks.flatMap((track) => track.clips);

  return (
    <div className="tab-content">
      <TypingSpeedSuggestions disabled={isExporting} />
      <div className="section">
        <div className="section-header">
          <h3>Clip Controls</h3>
          <p>Adjust speed, cursor visibility, and audio for each segment</p>
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
                  <div className="clip-actions">
                    <div className="clip-speed-badge">
                      <FastForward size={12} />
                      {clip.playbackRate}x
                    </div>
                    <button
                      className="clip-delete-btn"
                      onClick={() => deleteClip(clip.id)}
                      title="Delete clip and close the gap"
                      disabled={isExporting || clip.locked || tracks.find(track => track.id === clip.trackId)?.locked
                        || ((clip.type === 'video' || clip.type === 'screen') && allClips.filter(other => other.trackId === clip.trackId).length <= 1)}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                </div>

                {(clip.type === 'video' || clip.type === 'screen') && (
                  <label className="clip-cursor-toggle">
                    <span>Hide cursor in this clip</span>
                    <input type="checkbox" checked={clip.hideCursor ?? false}
                      onChange={(event) => updateClip(clip.id, { hideCursor: event.target.checked })}
                      disabled={isExporting || clip.locked} />
                  </label>
                )}
                {(clip.type === 'video' || clip.type === 'screen') && (
                  <label className="clip-cursor-toggle">
                    <span>Disable cursor smoothing in this clip</span>
                    <input type="checkbox" checked={clip.disableCursorSmoothing ?? false}
                      onChange={event => updateClip(clip.id, { disableCursorSmoothing: event.target.checked })}
                      disabled={isExporting || clip.locked || tracks.find(track => track.id === clip.trackId)?.locked} />
                  </label>
                )}
                {(clip.type === 'video' || clip.type === 'screen') && (
                  <>
                    {(['micVolume', 'systemVolume'] as const).filter(key => key === 'micVolume' ? hasMicrophone : hasSystemAudio).map(key => (
                      <div className="control-group" key={key}>
                        <label htmlFor={`${clip.id}-${key}`}>{key === 'micVolume' ? 'Microphone volume' : 'System audio volume'}</label>
                        <div className="slider-control">
                          <input id={`${clip.id}-${key}`} type="range" min="0" max="1" step="0.1"
                            value={clip[key] ?? 1} className="editor-slider"
                            onChange={event => updateClip(clip.id, { [key]: Number(event.target.value) })}
                            disabled={isExporting || clip.locked} />
                          <span className="value-display">{Math.round((clip[key] ?? 1) * 100)}%</span>
                        </div>
                      </div>
                    ))}
                  </>
                )}
                <div className="speed-presets">
                  {(['start', 'end', undefined] as const).map(side => (
                    <button key={side ?? 'both'} className="speed-preset-btn"
                      disabled={isExporting || !restoredClipBounds(allClips, tracks, clip.id, duration, videoFilePath, side)}
                      onClick={() => resetClipTrim(clip.id, side)}>
                      {side ? `Restore ${side}` : 'Reset trims'}
                    </button>
                  ))}
                </div>
                <div className="speed-presets">
                  {(['previous', 'next'] as const).map(direction => (
                    <button key={direction} className="speed-preset-btn"
                      title="Keeps this clip’s speed and settings and restores any trimmed source between the clips"
                      disabled={isExporting || !mergeNeighbor(allClips, tracks, clip.id, direction)}
                      onClick={() => mergeClip(clip.id, direction)}>
                      Merge with {direction}
                    </button>
                  ))}
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

                <button className="speed-preset-btn"
                  onClick={() => applyClipSpeedToTrack(clip.id)}
                  disabled={isExporting || clip.locked || tracks.find(track => track.id === clip.trackId)?.locked
                    || !allClips.some(other => other.trackId === clip.trackId && !other.locked && other.playbackRate !== clip.playbackRate)}>
                  Apply speed to all clips in this track
                </button>

                <div className="control-group">
                  <label>Custom Speed</label>
                  <div className="slider-control">
                    <input
                      type="range"
                      min="0.25"
                      max="24"
                      step="0.05"
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
          <p>• Press C for scissors, then click the timeline to split clips and zoom blocks</p>
          <p>• Select a clip or zoom block, then press Delete or Backspace to remove it</p>
          <p>• Speed up boring sections with 2x-4x</p>
          <p>• Slow down important actions with 0.5x</p>
        </div>
      </div>
    </div>
  );
};

export default ClipsTab;
