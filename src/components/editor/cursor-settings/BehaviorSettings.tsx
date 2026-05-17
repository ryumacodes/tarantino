import React, { useState } from 'react';
import { ChevronUp, RotateCcw } from 'lucide-react';
import type { VisualSettings } from '../../../stores/editor';

interface BehaviorSettingsProps {
  visualSettings: VisualSettings;
  updateVisualSettings: (settings: Partial<VisualSettings>) => void;
  isExporting?: boolean;
}

export const BehaviorSettings: React.FC<BehaviorSettingsProps> = ({
  visualSettings,
  updateVisualSettings,
  isExporting = false
}) => {
  const [rotationExpanded, setRotationExpanded] = useState(false);
  const [advancedExpanded, setAdvancedExpanded] = useState(false);

  return (
    <>
      {/* Rotation Section (Collapsible) */}
      <div className="collapsible-section">
        <button
          className="section-header"
          onClick={() => setRotationExpanded(!rotationExpanded)}
          data-testid="rotation-section-toggle"
        >
          <span className="section-title">Rotation</span>
          <ChevronUp
            size={20}
            className={`chevron ${rotationExpanded ? 'expanded' : ''}`}
          />
        </button>

        {rotationExpanded && (
          <div className="section-content">
            {/* Rotate cursor */}
            <div className="cursor-setting-section">
              <label className="cursor-setting-label">Rotate cursor</label>
              <div className="slider-with-reset">
                <input
                  type="range"
                  min="0"
                  max="360"
                  step="1"
                  value={visualSettings.cursorRotation}
                  onChange={(e) => updateVisualSettings({ cursorRotation: parseInt(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                  data-testid="cursor-rotation-slider"
                />
                <button
                  className="reset-btn"
                  onClick={() => updateVisualSettings({ cursorRotation: 0 })}
                  disabled={isExporting}
                >
                  Reset
                </button>
              </div>
            </div>

            {/* Rotate cursor while moving */}
            <div className="cursor-setting-section">
              <div className="toggle-row">
                <div className="toggle-info">
                  <div className="toggle-label-with-icon">
                    <RotateCcw size={16} />
                    <label className="cursor-setting-label">Rotate cursor while moving</label>
                  </div>
                  <span className="toggle-description">
                    If cursor is moving horizontally, it will slightly rotate as if it is chasing the mouse.
                  </span>
                </div>
                <label className="toggle-switch">
                  <input
                    type="checkbox"
                    checked={visualSettings.rotateCursorWhileMoving}
                    onChange={(e) => updateVisualSettings({ rotateCursorWhileMoving: e.target.checked })}
                    disabled={isExporting}
                  />
                  <span className="toggle-slider" />
                </label>
              </div>

              {visualSettings.rotateCursorWhileMoving && (
                <div className="slider-with-reset" style={{ marginTop: '12px' }}>
                  <input
                    type="range"
                    min="0"
                    max="100"
                    step="5"
                    value={visualSettings.rotationIntensity}
                    onChange={(e) => updateVisualSettings({ rotationIntensity: parseInt(e.target.value) })}
                    className="editor-slider"
                    disabled={isExporting}
                  />
                  <button
                    className="reset-btn"
                    onClick={() => updateVisualSettings({ rotationIntensity: 30 })}
                    disabled={isExporting}
                  >
                    Reset
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="cursor-divider" />

      {/* Advanced Section (Collapsible) */}
      <div className="collapsible-section">
        <button
          className="section-header"
          onClick={() => setAdvancedExpanded(!advancedExpanded)}
          data-testid="advanced-section-toggle"
        >
          <span className="section-title">Advanced</span>
          <ChevronUp
            size={20}
            className={`chevron ${advancedExpanded ? 'expanded' : ''}`}
          />
        </button>

        {advancedExpanded && (
          <div className="section-content">
            {/* Stop cursor movement at end */}
            <div className="cursor-setting-section">
              <label className="cursor-setting-label">Stop cursor movement at the end of the video</label>
              <span className="setting-description">
                Near the end of the video, the last mouse movement often leads to clicking "Stop Recording," which you might not want to be visible. Adjust how long before the end of the video the mouse cursor will stop moving.
              </span>
              <div className="slider-with-reset" style={{ marginTop: '12px' }}>
                <input
                  type="range"
                  min="0"
                  max="5000"
                  step="100"
                  value={visualSettings.stopCursorDuration}
                  onChange={(e) => updateVisualSettings({
                    stopCursorAtEnd: parseInt(e.target.value) > 0,
                    stopCursorDuration: parseInt(e.target.value)
                  })}
                  className="editor-slider"
                  disabled={isExporting}
                  data-testid="stop-cursor-slider"
                />
                <button
                  className="reset-btn"
                  onClick={() => updateVisualSettings({ stopCursorAtEnd: false, stopCursorDuration: 0 })}
                  disabled={isExporting}
                >
                  Reset
                </button>
              </div>
            </div>

            {/* Remove cursor shakes */}
            <div className="cursor-setting-section">
              <div className="toggle-row">
                <div className="toggle-info">
                  <label className="cursor-setting-label">Remove cursor shakes</label>
                  <span className="toggle-description">
                    If you use some accessibility apps that can control your mouse, it is possible those apps will cause sudden, short movements of your mouse. This option allows trying to detect and remove them.
                  </span>
                </div>
                <label className="toggle-switch">
                  <input
                    type="checkbox"
                    checked={visualSettings.removeCursorShakes}
                    onChange={(e) => updateVisualSettings({ removeCursorShakes: e.target.checked })}
                    disabled={isExporting}
                    data-testid="remove-shakes-toggle"
                  />
                  <span className="toggle-slider" />
                </label>
              </div>

              {visualSettings.removeCursorShakes && (
                <>
                  <label className="cursor-setting-label" style={{ marginTop: '16px' }}>
                    Remove cursor shakes threshold
                  </label>
                  <div className="slider-with-reset">
                    <input
                      type="range"
                      min="1"
                      max="20"
                      step="1"
                      value={visualSettings.shakesThreshold}
                      onChange={(e) => updateVisualSettings({ shakesThreshold: parseInt(e.target.value) })}
                      className="editor-slider"
                      disabled={isExporting}
                    />
                    <button
                      className="reset-btn"
                      onClick={() => updateVisualSettings({ shakesThreshold: 3 })}
                      disabled={isExporting}
                    >
                      Reset
                    </button>
                  </div>
                </>
              )}
            </div>

            {/* Optimize cursor changes */}
            <div className="cursor-setting-section">
              <div className="toggle-row">
                <div className="toggle-info">
                  <label className="cursor-setting-label">Optimize cursor changes</label>
                  <span className="toggle-description">
                    Will try to minimize rapid cursor changes (eg when quickly moving over some elements) when this option is enabled.
                  </span>
                </div>
                <label className="toggle-switch">
                  <input
                    type="checkbox"
                    checked={visualSettings.optimizeCursorChanges}
                    onChange={(e) => updateVisualSettings({ optimizeCursorChanges: e.target.checked })}
                    disabled={isExporting}
                  />
                  <span className="toggle-slider" />
                </label>
              </div>
            </div>
          </div>
        )}
      </div>
    </>
  );
};

export default BehaviorSettings;
