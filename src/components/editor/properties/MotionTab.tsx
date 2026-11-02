import { useShallow } from 'zustand/react/shallow';
import React from 'react';
import { useEditorStore, SPRING_PRESETS, type SpringPreset } from '../../../stores/editor';
import { SCREEN_MOTION_OPTIONS, resolveScreenSpring } from '../../../utils/motion/screenStyle';
import { CURSOR_MOTION_OPTIONS, resolveCursorSpring } from '../../../utils/motion/cursorStyle';
import type { TabProps } from './types';

const springPresets: { preset: SpringPreset; label: string; description: string }[] = [
  { preset: 'rapid', label: 'Rapid', description: 'Near-instant' },
  { preset: 'quick', label: 'Quick', description: 'Snappy, responsive' },
  { preset: 'mellow', label: 'Mellow', description: 'Balanced, default' },
  { preset: 'slow', label: 'Slow', description: 'Smooth, cinematic' },
];

const MotionTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { visualSettings, updateVisualSettings } = useEditorStore(useShallow(state => ({
    visualSettings: state.visualSettings,
    updateVisualSettings: state.updateVisualSettings,
  })));

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Zoom Animation</h3>
          <p>Spring physics for zoom transitions</p>
        </div>

        <div className="control-group">
          <label htmlFor="zoom-lead-in">Zoom lead-in: {visualSettings.zoomLeadInMs ?? 0} ms</label>
          <input id="zoom-lead-in" type="range" min={0} max={2000} step={50}
            value={visualSettings.zoomLeadInMs ?? 0} disabled={isExporting}
            onChange={event => updateVisualSettings({ zoomLeadInMs: Number(event.target.value) })} />
          <small className="setting-hint">Begin zooming before each range. Zero starts at the range boundary. Instant zooms and existing zoom ranges take priority.</small>
        </div>
        <div className="zoom-presets-grid">
          {SCREEN_MOTION_OPTIONS.map(({ preset, label, description }) => (
            <button key={preset}
              className={`zoom-preset-btn ${visualSettings.screenMotionStyle === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ screenMotionStyle: preset })}
              disabled={isExporting}>
              <span className="preset-label">{label}</span>
              <span className="preset-desc">{description}</span>
            </button>
          ))}
        </div>
        <p className="setting-hint">Or use a legacy speed preset:</p>
        <div className="zoom-presets-grid">
          {springPresets.map(({ preset, label, description }) => (
            <button
              key={preset}
              className={`zoom-preset-btn ${!visualSettings.screenMotionStyle && visualSettings.zoomSpeedPreset === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ zoomSpeedPreset: preset, screenMotionStyle: undefined })}
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
          {CURSOR_MOTION_OPTIONS.map(({ preset, label, description }) => (
            <button
              key={preset}
              className={`zoom-preset-btn ${visualSettings.cursorMotionStyle === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ cursorMotionStyle: preset })}
              disabled={isExporting}
            >
              <span className="preset-label">{label}</span>
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
            <span className="info-value">{resolveScreenSpring(visualSettings).tension}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Zoom Friction</span>
            <span className="info-value">{resolveScreenSpring(visualSettings).friction}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Cursor Tension</span>
            <span className="info-value">{resolveCursorSpring(visualSettings).tension}</span>
          </div>
          <div className="info-row">
            <span className="info-label">Cursor Friction</span>
            <span className="info-value">{resolveCursorSpring(visualSettings).friction}</span>
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
