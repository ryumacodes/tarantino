import { useShallow } from 'zustand/react/shallow';
import { CameraTimelineControls } from './CameraTimelineControls';
import React from 'react';
import { Camera } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import { StyleSettings } from './StyleSettings';
import { BehaviorSettings } from './BehaviorSettings';
import './cursor-settings.css';

const webcamCorners = [
  { id: 'top-left' as const, label: 'TL' },
  { id: 'top-right' as const, label: 'TR' },
  { id: 'bottom-left' as const, label: 'BL' },
  { id: 'bottom-right' as const, label: 'BR' },
];

const webcamShapes = [
  { id: 'circle' as const, label: 'Circle' },
  { id: 'roundrect' as const, label: 'Rounded' },
];

const webcamCornerPosition = (
  corner: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right',
  size: number
) => {
  const near = Math.min(0.95, Math.max(0.05, 0.03 + size / 2));
  const far = 1 - near;
  switch (corner) {
    case 'top-left':
      return { webcamX: near, webcamY: near };
    case 'top-right':
      return { webcamX: far, webcamY: near };
    case 'bottom-left':
      return { webcamX: near, webcamY: far };
    default:
      return { webcamX: far, webcamY: far };
  }
};

interface CursorSettingsPanelProps {
  onShowMouseOverlay?: (show: boolean) => void;
  showMouseOverlay?: boolean;
  isExporting?: boolean;
}

export const CursorSettingsPanel: React.FC<CursorSettingsPanelProps> = ({
  onShowMouseOverlay,
  showMouseOverlay = true,
  isExporting = false
}) => {
  const { visualSettings, updateVisualSettings, hasWebcam } = useEditorStore(useShallow(state => ({
    visualSettings: state.visualSettings,
    updateVisualSettings: state.updateVisualSettings,
    hasWebcam: state.hasWebcam,
  })));
  const webcamX = visualSettings.webcamX ?? 0.895;
  const webcamY = visualSettings.webcamY ?? 0.895;

  return (
    <div className="cursor-settings-panel" data-testid="cursor-settings-panel">
      <StyleSettings
        visualSettings={visualSettings}
        updateVisualSettings={updateVisualSettings}
        isExporting={isExporting}
      />

      <div className="cursor-divider" />

      <BehaviorSettings
        visualSettings={visualSettings}
        updateVisualSettings={updateVisualSettings}
        isExporting={isExporting}
      />

      {hasWebcam && (
        <>
          <div className="cursor-divider" />

          <CameraTimelineControls disabled={isExporting} />
          <div className="cursor-setting-section">
            <div className="toggle-label-with-icon">
              <Camera size={14} />
              <label className="cursor-setting-label">Camera position</label>
            </div>
            <div className="click-effect-group" style={{ marginBottom: 10 }}>
              {webcamCorners.map((c) => (
                <button
                  key={c.id}
                  className={`click-effect-btn ${visualSettings.webcamCorner === c.id ? 'active' : ''}`}
                  onClick={() => updateVisualSettings({
                    webcamCorner: c.id,
                    ...webcamCornerPosition(c.id, visualSettings.webcamSize),
                  })}
                  disabled={isExporting}
                >
                  {c.label}
                </button>
              ))}
            </div>

            <label className="cursor-setting-label">Horizontal</label>
            <div className="slider-with-reset">
              <input
                type="range"
                min="0.05"
                max="0.95"
                step="0.01"
                value={webcamX}
                onChange={(e) => updateVisualSettings({ webcamX: parseFloat(e.target.value) })}
                disabled={isExporting}
                style={{ flex: 1 }}
              />
              <span className="value-display">{Math.round(webcamX * 100)}%</span>
            </div>

            <label className="cursor-setting-label">Vertical</label>
            <div className="slider-with-reset" style={{ marginBottom: 10 }}>
              <input
                type="range"
                min="0.05"
                max="0.95"
                step="0.01"
                value={webcamY}
                onChange={(e) => updateVisualSettings({ webcamY: parseFloat(e.target.value) })}
                disabled={isExporting}
                style={{ flex: 1 }}
              />
              <span className="value-display">{Math.round(webcamY * 100)}%</span>
            </div>

            <label className="cursor-setting-label">Shape</label>
            <div className="click-effect-group" style={{ marginBottom: 10 }}>
              {webcamShapes.map((s) => (
                <button
                  key={s.id}
                  className={`click-effect-btn ${visualSettings.webcamShape === s.id ? 'active' : ''}`}
                  onClick={() => updateVisualSettings({ webcamShape: s.id })}
                  disabled={isExporting}
                >
                  {s.label}
                </button>
              ))}
            </div>

            <label className="cursor-setting-label">Size</label>
            <div className="slider-with-reset">
              <input
                type="range"
                min="0.08"
                max="0.25"
                step="0.01"
                value={visualSettings.webcamSize}
                onChange={(e) => updateVisualSettings({ webcamSize: parseFloat(e.target.value) })}
                disabled={isExporting}
                style={{ flex: 1 }}
              />
              <button
                className="reset-btn"
                onClick={() => updateVisualSettings({
                  webcamSize: 0.15,
                  ...webcamCornerPosition(visualSettings.webcamCorner, 0.15),
                })}
                disabled={isExporting}
              >
                Reset
              </button>
            </div>
          </div>
        </>
      )}


    </div>
  );
};

export default CursorSettingsPanel;
