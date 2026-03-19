import React from 'react';
import { Monitor, Palette, Layers } from 'lucide-react';
import { useEditorStore, WALLPAPERS, type BackgroundType, type GradientDirection } from '../../../stores/editor';
import type { TabProps } from './types';

const backgroundTypes: { type: BackgroundType; label: string; icon: React.ReactNode }[] = [
  { type: 'solid', label: 'Solid', icon: <Monitor size={16} /> },
  { type: 'gradient', label: 'Gradient', icon: <Palette size={16} /> },
  { type: 'wallpaper', label: 'Preset', icon: <Layers size={16} /> },
];

const gradientDirections: { dir: GradientDirection; label: string }[] = [
  { dir: 'to-right', label: '→' },
  { dir: 'to-bottom', label: '↓' },
  { dir: 'to-bottom-right', label: '↘' },
  { dir: 'radial', label: '◉' },
];

const BackgroundTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { visualSettings, updateVisualSettings, applyWallpaper } = useEditorStore();

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Background Style</h3>
          <p>Choose how your recording appears</p>
        </div>

        <div className="background-types">
          {backgroundTypes.map(({ type, label, icon }) => (
            <button
              key={type}
              className={`background-type ${visualSettings.backgroundType === type ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ backgroundType: type })}
              disabled={isExporting}
            >
              {icon}
              <span>{label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Solid Color */}
      {visualSettings.backgroundType === 'solid' && (
        <div className="section">
          <div className="section-header">
            <h3>Background Color</h3>
          </div>
          <div className="control-group">
            <label>Color</label>
            <div className="color-picker-row">
              <input
                type="color"
                value={visualSettings.backgroundColor}
                onChange={(e) => updateVisualSettings({ backgroundColor: e.target.value })}
                className="color-input"
                disabled={isExporting}
              />
              <span className="color-value">{visualSettings.backgroundColor}</span>
            </div>
          </div>
        </div>
      )}

      {/* Gradient */}
      {visualSettings.backgroundType === 'gradient' && (
        <div className="section">
          <div className="section-header">
            <h3>Gradient Settings</h3>
          </div>

          <div className="control-group">
            <label>Direction</label>
            <div className="direction-buttons">
              {gradientDirections.map(({ dir, label }) => (
                <button
                  key={dir}
                  className={`dir-btn ${visualSettings.gradientDirection === dir ? 'active' : ''}`}
                  onClick={() => updateVisualSettings({ gradientDirection: dir })}
                  disabled={isExporting}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          <div className="control-group">
            <label>Colors</label>
            {visualSettings.gradientStops.map((stop, i) => (
              <div key={i} className="color-picker-row">
                <input
                  type="color"
                  value={stop.color}
                  onChange={(e) => {
                    const newStops = [...visualSettings.gradientStops];
                    newStops[i] = { ...newStops[i], color: e.target.value };
                    updateVisualSettings({ gradientStops: newStops });
                  }}
                  className="color-input"
                  disabled={isExporting}
                />
                <span className="color-value">{stop.color}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Wallpaper Presets */}
      {visualSettings.backgroundType === 'wallpaper' && (
        <div className="section">
          <div className="section-header">
            <h3>Background Presets</h3>
          </div>
          <div className="wallpaper-grid">
            {Object.entries(WALLPAPERS).map(([id, wallpaper]) => (
              <button
                key={id}
                className={`wallpaper-item ${visualSettings.wallpaperId === id ? 'active' : ''}`}
                onClick={() => applyWallpaper(id)}
                disabled={isExporting}
              >
                <div
                  className="wallpaper-preview"
                  style={{
                    background: wallpaper.type === 'gradient'
                      ? `linear-gradient(135deg, ${(wallpaper as any).colors.join(', ')})`
                      : (wallpaper as any).color,
                  }}
                />
                <span className="wallpaper-name">{id.replace('gradient-', '').replace('solid-', '')}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="section">
        <div className="section-header">
          <h3>Frame Settings</h3>
        </div>

        <div className="control-group">
          <label>Padding</label>
          <div className="slider-control">
            <input
              type="range"
              min="0"
              max="50"
              step="1"
              value={visualSettings.padding}
              onChange={(e) => updateVisualSettings({ padding: parseInt(e.target.value) })}
              className="editor-slider"
              disabled={isExporting}
            />
            <span className="value-display">{visualSettings.padding}%</span>
          </div>
        </div>

        <div className="control-group">
          <label>Corner Radius</label>
          <div className="slider-control">
            <input
              type="range"
              min="0"
              max="50"
              step="1"
              value={visualSettings.cornerRadius}
              onChange={(e) => updateVisualSettings({ cornerRadius: parseInt(e.target.value) })}
              className="editor-slider"
              disabled={isExporting}
            />
            <span className="value-display">{visualSettings.cornerRadius}px</span>
          </div>
        </div>

        <div className="control-group">
          <label>Inset</label>
          <div className="slider-control">
            <input
              type="range"
              min="0"
              max="20"
              step="1"
              value={visualSettings.inset}
              onChange={(e) => updateVisualSettings({ inset: parseInt(e.target.value) })}
              className="editor-slider"
              disabled={isExporting}
            />
            <span className="value-display">{visualSettings.inset}px</span>
          </div>
        </div>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Shadow</h3>
        </div>

        <div className="control-group">
          <div className="checkbox-control">
            <input
              type="checkbox"
              id="shadow-enabled"
              checked={visualSettings.shadowEnabled}
              onChange={(e) => updateVisualSettings({ shadowEnabled: e.target.checked })}
              disabled={isExporting}
            />
            <label htmlFor="shadow-enabled">Enable Shadow</label>
          </div>
        </div>

        {visualSettings.shadowEnabled && (
          <>
            <div className="control-group">
              <label>Intensity</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="5"
                  value={visualSettings.shadowIntensity}
                  onChange={(e) => updateVisualSettings({ shadowIntensity: parseInt(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{visualSettings.shadowIntensity}%</span>
              </div>
            </div>

            <div className="control-group">
              <label>Blur</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="5"
                  value={visualSettings.shadowBlur}
                  onChange={(e) => updateVisualSettings({ shadowBlur: parseInt(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{visualSettings.shadowBlur}%</span>
              </div>
            </div>

            <div className="control-group">
              <label>Offset Y</label>
              <div className="slider-control">
                <input
                  type="range"
                  min="-50"
                  max="50"
                  step="1"
                  value={visualSettings.shadowOffsetY}
                  onChange={(e) => updateVisualSettings({ shadowOffsetY: parseInt(e.target.value) })}
                  className="editor-slider"
                  disabled={isExporting}
                />
                <span className="value-display">{visualSettings.shadowOffsetY}</span>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default BackgroundTab;
