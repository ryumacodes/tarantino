import React from 'react';
import { useEditorStore } from '../../../stores/editor';
import type { TabProps } from './types';

const AudioTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { audioSettings, updateAudioSettings } = useEditorStore();

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Audio Levels</h3>
        </div>

        <div className="control-group">
          <label>Microphone Gain</label>
          <div className="slider-control">
            <input
              type="range"
              min="-20"
              max="20"
              step="1"
              value={audioSettings.micGain}
              onChange={(e) => updateAudioSettings({
                micGain: parseInt(e.target.value)
              })}
              className="editor-slider"
              disabled={isExporting}
            />
            <span className="value-display">
              {audioSettings.micGain > 0 ? '+' : ''}{audioSettings.micGain} dB
            </span>
          </div>
        </div>

        <div className="control-group">
          <label>System Audio Gain</label>
          <div className="slider-control">
            <input
              type="range"
              min="-20"
              max="20"
              step="1"
              value={audioSettings.systemGain}
              onChange={(e) => updateAudioSettings({
                systemGain: parseInt(e.target.value)
              })}
              className="editor-slider"
              disabled={isExporting}
            />
            <span className="value-display">
              {audioSettings.systemGain > 0 ? '+' : ''}{audioSettings.systemGain} dB
            </span>
          </div>
        </div>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Audio Processing</h3>
        </div>

        <div className="control-group">
          <div className="checkbox-control">
            <input
              type="checkbox"
              id="noise-gate"
              checked={audioSettings.noiseGate}
              onChange={(e) => updateAudioSettings({
                noiseGate: e.target.checked
              })}
              disabled={isExporting}
            />
            <label htmlFor="noise-gate">Noise Reduction</label>
          </div>
        </div>

        <div className="control-group">
          <div className="checkbox-control">
            <input
              type="checkbox"
              id="dual-track"
              checked={audioSettings.dualTrack}
              onChange={(e) => updateAudioSettings({
                dualTrack: e.target.checked
              })}
              disabled={isExporting}
            />
            <label htmlFor="dual-track">Dual Track Export</label>
          </div>
        </div>
      </div>
    </div>
  );
};

export default AudioTab;
