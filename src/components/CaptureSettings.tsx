import React, { useState } from 'react';
import { 
  MousePointer2, 
  Film, 
  Maximize2, 
  Settings2,
  ChevronDown 
} from 'lucide-react';
import { cn } from '../utils/cn';

export interface CaptureConfig {
  includeCursor: boolean;
  cursorSize: 'small' | 'normal' | 'large';
  highlightClicks: boolean;
  fps: 30 | 60 | 120;
  outputResolution: 'match' | '1080p' | '1440p' | '4k';
  encoder: 'auto' | 'h264' | 'h265' | 'prores';
  container: 'mp4' | 'mov';
}

interface CaptureSettingsProps {
  config: CaptureConfig;
  onChange: (config: CaptureConfig) => void;
  sourceResolution?: { width: number; height: number };
  compact?: boolean;
}

const CaptureSettings: React.FC<CaptureSettingsProps> = ({
  config,
  onChange,
  sourceResolution,
  compact = false
}) => {
  const [expanded, setExpanded] = useState(false);

  const updateConfig = (updates: Partial<CaptureConfig>) => {
    onChange({ ...config, ...updates });
  };

  const resolutionOptions = [
    { value: 'match', label: 'Match Display', resolution: sourceResolution },
    { value: '1080p', label: '1080p', resolution: { width: 1920, height: 1080 } },
    { value: '1440p', label: '1440p', resolution: { width: 2560, height: 1440 } },
    { value: '4k', label: '4K', resolution: { width: 3840, height: 2160 } }
  ];

  const getOutputResolution = () => {
    const option = resolutionOptions.find(o => o.value === config.outputResolution);
    if (!option?.resolution) return 'Unknown';
    return `${option.resolution.width}×${option.resolution.height}`;
  };

  if (compact) {
    return (
      <div className="capture-settings-compact">
        <button
          className="capture-settings-compact__trigger"
          onClick={() => setExpanded(!expanded)}
        >
          <Settings2 size={16} />
          <span>{config.fps}fps • {config.outputResolution}</span>
          <ChevronDown size={14} />
        </button>

        {expanded && (
          <div className="capture-settings-compact__dropdown">
            <div className="capture-settings-compact__section">
              <label>Frame Rate</label>
              <div className="capture-settings-compact__fps">
                {[30, 60, 120].map(fps => (
                  <button
                    key={fps}
                    className={cn('capture-settings-compact__fps-option', {
                      active: config.fps === fps
                    })}
                    onClick={() => updateConfig({ fps: fps as 30 | 60 | 120 })}
                  >
                    {fps}
                  </button>
                ))}
              </div>
            </div>

            <div className="capture-settings-compact__section">
              <label>Resolution</label>
              <select
                value={config.outputResolution}
                onChange={(e) => updateConfig({ 
                  outputResolution: e.target.value as CaptureConfig['outputResolution'] 
                })}
              >
                {resolutionOptions.map(opt => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label} {opt.resolution && `(${opt.resolution.width}×${opt.resolution.height})`}
                  </option>
                ))}
              </select>
            </div>

            <div className="capture-settings-compact__section">
              <label className="capture-settings-compact__checkbox">
                <input
                  type="checkbox"
                  checked={config.includeCursor}
                  onChange={(e) => updateConfig({ includeCursor: e.target.checked })}
                />
                <MousePointer2 size={14} />
                <span>Include Cursor</span>
              </label>

              {config.includeCursor && (
                <label className="capture-settings-compact__checkbox">
                  <input
                    type="checkbox"
                    checked={config.highlightClicks}
                    onChange={(e) => updateConfig({ highlightClicks: e.target.checked })}
                  />
                  <span>Highlight Clicks</span>
                </label>
              )}
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="capture-settings">
      <div className="capture-settings__section">
        <h4>
          <Film size={16} />
          Recording
        </h4>
        
        <div className="capture-settings__field">
          <label>Frame Rate</label>
          <div className="capture-settings__fps-selector">
            {[30, 60, 120].map(fps => (
              <button
                key={fps}
                className={cn('capture-settings__fps-option', {
                  active: config.fps === fps
                })}
                onClick={() => updateConfig({ fps: fps as 30 | 60 | 120 })}
              >
                <span className="capture-settings__fps-value">{fps}</span>
                <span className="capture-settings__fps-label">fps</span>
              </button>
            ))}
          </div>
        </div>

        <div className="capture-settings__field">
          <label>
            <Maximize2 size={14} />
            Output Resolution
          </label>
          <select
            value={config.outputResolution}
            onChange={(e) => updateConfig({ 
              outputResolution: e.target.value as CaptureConfig['outputResolution'] 
            })}
            className="capture-settings__select"
          >
            {resolutionOptions.map(opt => (
              <option key={opt.value} value={opt.value}>
                {opt.label} {opt.resolution && `(${opt.resolution.width}×${opt.resolution.height})`}
              </option>
            ))}
          </select>
          <div className="capture-settings__hint">
            Output: {getOutputResolution()}
          </div>
        </div>
      </div>

      <div className="capture-settings__section">
        <h4>
          <MousePointer2 size={16} />
          Cursor
        </h4>
        
        <label className="capture-settings__checkbox">
          <input
            type="checkbox"
            checked={config.includeCursor}
            onChange={(e) => updateConfig({ includeCursor: e.target.checked })}
          />
          <span>Include cursor in recording</span>
        </label>

        {config.includeCursor && (
          <>
            <label className="capture-settings__checkbox">
              <input
                type="checkbox"
                checked={config.highlightClicks}
                onChange={(e) => updateConfig({ highlightClicks: e.target.checked })}
              />
              <span>Highlight clicks</span>
            </label>

            <div className="capture-settings__field">
              <label>Cursor Size</label>
              <select
                value={config.cursorSize}
                onChange={(e) => updateConfig({ 
                  cursorSize: e.target.value as CaptureConfig['cursorSize'] 
                })}
                className="capture-settings__select"
              >
                <option value="small">Small (0.8x)</option>
                <option value="normal">Normal (1x)</option>
                <option value="large">Large (1.5x)</option>
              </select>
            </div>
          </>
        )}
      </div>

      <div className="capture-settings__section">
        <h4>Format</h4>
        
        <div className="capture-settings__field">
          <label>Encoder</label>
          <select
            value={config.encoder}
            onChange={(e) => updateConfig({ 
              encoder: e.target.value as CaptureConfig['encoder'] 
            })}
            className="capture-settings__select"
          >
            <option value="auto">Auto (Hardware if available)</option>
            <option value="h264">H.264</option>
            <option value="h265">H.265/HEVC</option>
            <option value="prores">ProRes 422</option>
          </select>
        </div>

        <div className="capture-settings__field">
          <label>Container</label>
          <select
            value={config.container}
            onChange={(e) => updateConfig({ 
              container: e.target.value as CaptureConfig['container'] 
            })}
            className="capture-settings__select"
          >
            <option value="mp4">MP4</option>
            <option value="mov">MOV</option>
          </select>
        </div>
      </div>
    </div>
  );
};

export default CaptureSettings;