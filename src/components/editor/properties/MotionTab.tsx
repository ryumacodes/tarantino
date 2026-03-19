import React from 'react';
import { useEditorStore, SPRING_PRESETS, type SpringPreset } from '../../../stores/editor';
import type { TabProps } from './types';

const springPresets: { preset: SpringPreset; label: string; description: string }[] = [
  { preset: 'rapid', label: 'Rapid', description: 'Near-instant' },
  { preset: 'quick', label: 'Quick', description: 'Snappy, responsive' },
  { preset: 'mellow', label: 'Mellow', description: 'Balanced, default' },
  { preset: 'slow', label: 'Slow', description: 'Smooth, cinematic' },
];

const MotionTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { visualSettings, updateVisualSettings } = useEditorStore();

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Zoom Animation</h3>
          <p>Spring physics for zoom transitions</p>
        </div>

        <div className="zoom-presets-grid">
          {springPresets.map(({ preset, label, description }) => (
            <button
              key={preset}
              className={`zoom-preset-btn ${visualSettings.zoomSpeedPreset === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ zoomSpeedPreset: preset })}
              disabled={isExporting}
            >
              <span className="preset-label">{label}</span>
              <span className="preset-spring">T:{SPRING_PRESETS[preset].tension}</span>
              <span className="preset-desc">{description}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Cursor Animation</h3>
          <p>Independent cursor following speed</p>
        </div>

        <div className="zoom-presets-grid">
          {springPresets.map(({ preset, label, description }) => (
            <button
              key={preset}
              className={`zoom-preset-btn ${visualSettings.cursorSpeedPreset === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ cursorSpeedPreset: preset })}
              disabled={isExporting}
            >
              <span className="preset-label">{label}</span>
              <span className="preset-spring">T:{SPRING_PRESETS[preset].tension}</span>
              <span className="preset-desc">{description}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Spring Physics Details</h3>
        </div>

        <div className="animation-info">
          <div className="info-row">
            <span className="info-label">Zoom Tension</span>
            <span className="info-value">{SPRING_PRESETS[visualSettings.zoomSpeedPreset].tension}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Zoom Friction</span>
            <span className="info-value">{SPRING_PRESETS[visualSettings.zoomSpeedPreset].friction}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Cursor Tension</span>
            <span className="info-value">{SPRING_PRESETS[visualSettings.cursorSpeedPreset].tension}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Cursor Friction</span>
            <span className="info-value">{SPRING_PRESETS[visualSettings.cursorSpeedPreset].friction}</span>
          </div>
        </div>
        <small className="setting-hint">Higher tension = snappier, higher friction = less bouncy</small>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Motion Blur</h3>
          <p>Velocity-based blur during animations</p>
        </div>

        <div className="control-group">
          <div className="checkbox-control">
            <input
              type="checkbox"
              id="motion-blur"
              checked={visualSettings.motionBlurEnabled}
              onChange={(e) => updateVisualSettings({ motionBlurEnabled: e.target.checked })}
              disabled={isExporting}
            />
            <label htmlFor="motion-blur">Enable Motion Blur</label>
          </div>
        </div>

        {visualSettings.motionBlurEnabled && (
          <>
            <div className="control-group">
              <label>Screen Moving</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  value={visualSettings.motionBlurPanIntensity}
                  onChange={(e) => updateVisualSettings({ motionBlurPanIntensity: parseFloat(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{Math.round(visualSettings.motionBlurPanIntensity)}%</span>
              </div>
            </div>
            <div className="control-group">
              <label>Screen Zooming</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  value={visualSettings.motionBlurZoomIntensity}
                  onChange={(e) => updateVisualSettings({ motionBlurZoomIntensity: parseFloat(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{Math.round(visualSettings.motionBlurZoomIntensity)}%</span>
              </div>
            </div>
            <div className="control-group">
              <label>Cursor Movement</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  value={visualSettings.motionBlurCursorIntensity}
                  onChange={(e) => updateVisualSettings({ motionBlurCursorIntensity: parseFloat(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{Math.round(visualSettings.motionBlurCursorIntensity)}%</span>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default MotionTab;
