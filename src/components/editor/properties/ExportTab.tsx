import React from 'react';
import { useEditorStore, ASPECT_RATIOS, RESOLUTION_DIMENSIONS, type AspectRatio, type ExportResolution, type ExportFrameRate, type ExportFormat } from '../../../stores/editor';
import type { TabProps } from './types';

const ExportTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { visualSettings, updateVisualSettings, exportSettings, updateExportSettings } = useEditorStore();

  return (
    <div className="tab-content">
      <div className="section">
        <div className="section-header">
          <h3>Aspect Ratio</h3>
          <p>Choose output dimensions</p>
        </div>

        <div className="aspect-ratio-grid">
          {(Object.entries(ASPECT_RATIOS) as [AspectRatio, typeof ASPECT_RATIOS[keyof typeof ASPECT_RATIOS]][]).map(([ratio, info]) => (
            <button
              key={ratio}
              className={`aspect-ratio-btn ${visualSettings.aspectRatio === ratio ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ aspectRatio: ratio })}
              disabled={isExporting}
            >
              <div className="aspect-preview" style={{
                aspectRatio: ratio === 'auto' ? '16/9' : `${info.width}/${info.height}`,
              }}>
                {ratio !== 'auto' && <span className="aspect-value">{ratio}</span>}
                {ratio === 'auto' && <span className="aspect-value">Auto</span>}
              </div>
              <span className="aspect-label">{info.label}</span>
              <span className="aspect-desc">{info.description}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="section">
        <div className="section-header">
          <h3>Export Settings</h3>
        </div>

        <div className="control-group">
          <label>Resolution</label>
          <select
            className="editor-input"
            value={exportSettings.resolution}
            onChange={(e) => updateExportSettings({ resolution: e.target.value as ExportResolution })}
            disabled={isExporting}
          >
            <option value="1080p">1080p (1920x1080)</option>
            <option value="4k">4K (3840x2160)</option>
            <option value="1440p">1440p (2560x1440)</option>
            <option value="720p">720p (1280x720)</option>
            <option value="custom">Custom</option>
          </select>
        </div>

        {exportSettings.resolution === 'custom' && (
          <div className="control-group custom-resolution">
            <div className="custom-res-inputs">
              <input
                type="number"
                className="editor-input editor-input--small"
                placeholder="Width"
                value={exportSettings.customWidth || ''}
                onChange={(e) => updateExportSettings({ customWidth: parseInt(e.target.value) || undefined })}
                disabled={isExporting}
              />
              <span className="res-separator">×</span>
              <input
                type="number"
                className="editor-input editor-input--small"
                placeholder="Height"
                value={exportSettings.customHeight || ''}
                onChange={(e) => updateExportSettings({ customHeight: parseInt(e.target.value) || undefined })}
                disabled={isExporting}
              />
            </div>
          </div>
        )}

        <div className="control-group">
          <label>Frame Rate</label>
          <select
            className="editor-input"
            value={exportSettings.frameRate}
            onChange={(e) => updateExportSettings({ frameRate: parseInt(e.target.value) as ExportFrameRate })}
            disabled={isExporting}
          >
            <option value={60}>60 FPS</option>
            <option value={30}>30 FPS</option>
            <option value={24}>24 FPS (Cinematic)</option>
          </select>
        </div>

        <div className="control-group">
          <label>Quality</label>
          <div className="quality-presets">
            <button
              className={`quality-preset ${exportSettings.quality === 'high' ? 'active' : ''}`}
              onClick={() => updateExportSettings({ quality: 'high' })}
              disabled={isExporting}
            >
              <span>High</span>
              <small>Best quality</small>
            </button>
            <button
              className={`quality-preset ${exportSettings.quality === 'medium' ? 'active' : ''}`}
              onClick={() => updateExportSettings({ quality: 'medium' })}
              disabled={isExporting}
            >
              <span>Medium</span>
              <small>Balanced</small>
            </button>
            <button
              className={`quality-preset ${exportSettings.quality === 'low' ? 'active' : ''}`}
              onClick={() => updateExportSettings({ quality: 'low' })}
              disabled={isExporting}
            >
              <span>Low</span>
              <small>Smaller file</small>
            </button>
          </div>
        </div>

        <div className="control-group">
          <label>Format</label>
          <select
            className="editor-input"
            value={exportSettings.format}
            onChange={(e) => updateExportSettings({ format: e.target.value as ExportFormat })}
            disabled={isExporting}
          >
            <option value="mp4">MP4 (H.264)</option>
            <option value="mov">MOV (ProRes)</option>
            <option value="webm">WebM (VP9)</option>
            <option value="gif">GIF (Animated)</option>
          </select>
        </div>
      </div>

      {/* Export Summary */}
      <div className="section">
        <div className="section-header">
          <h3>Export Summary</h3>
        </div>
        <div className="export-summary">
          <div className="summary-row">
            <span className="summary-label">Output</span>
            <span className="summary-value">
              {exportSettings.resolution === 'custom'
                ? `${exportSettings.customWidth || '?'}×${exportSettings.customHeight || '?'}`
                : `${RESOLUTION_DIMENSIONS[exportSettings.resolution as keyof typeof RESOLUTION_DIMENSIONS]?.width}×${RESOLUTION_DIMENSIONS[exportSettings.resolution as keyof typeof RESOLUTION_DIMENSIONS]?.height}`
              } @ {exportSettings.frameRate}fps
            </span>
          </div>
          <div className="summary-row">
            <span className="summary-label">Format</span>
            <span className="summary-value">{exportSettings.format.toUpperCase()}</span>
          </div>
          <div className="summary-row">
            <span className="summary-label">Quality</span>
            <span className="summary-value">{exportSettings.quality.charAt(0).toUpperCase() + exportSettings.quality.slice(1)}</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ExportTab;
