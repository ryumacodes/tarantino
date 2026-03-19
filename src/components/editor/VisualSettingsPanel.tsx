import React from 'react';
import {
  useEditorStore,
  WALLPAPERS,
  ZOOM_SPEED_PRESETS,
  ASPECT_RATIOS,
  DEVICE_FRAMES,
  type BackgroundType,
  type GradientDirection,
  type AspectRatio,
  type DeviceFrame,
  type VisualSettings,
} from '../../stores/editor';
import '../../styles/visual-settings-panel.css';
import {
  Palette,
  Square,
  Circle,
  Sun,
  Move,
  Maximize,
  CornerUpRight,
  Moon,
  MousePointer,
  Zap,
  RotateCcw,
  Monitor,
  Smartphone,
  Laptop,
  Globe,
  Tablet,
} from 'lucide-react';

interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  unit?: string;
}

const Slider: React.FC<SliderProps> = ({ label, value, min, max, step = 1, onChange, unit = '' }) => (
  <div className="settings-slider">
    <div className="slider-header">
      <span className="slider-label">{label}</span>
      <span className="slider-value">{value}{unit}</span>
    </div>
    <input
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      className="slider-input"
    />
  </div>
);

interface ColorPickerProps {
  label: string;
  value: string;
  onChange: (color: string) => void;
}

const ColorPicker: React.FC<ColorPickerProps> = ({ label, value, onChange }) => (
  <div className="settings-color-picker">
    <span className="color-label">{label}</span>
    <div className="color-input-wrapper">
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="color-input"
      />
      <span className="color-value">{value}</span>
    </div>
  </div>
);

const VisualSettingsPanel: React.FC = () => {
  const { visualSettings, updateVisualSettings, applyWallpaper, resetVisualSettings } = useEditorStore();

  const backgroundTypes: { type: BackgroundType; label: string; icon: React.ReactNode }[] = [
    { type: 'solid', label: 'Solid', icon: <Square size={14} /> },
    { type: 'gradient', label: 'Gradient', icon: <Palette size={14} /> },
    { type: 'wallpaper', label: 'Wallpaper', icon: <Sun size={14} /> },
  ];

  const gradientDirections: { dir: GradientDirection; label: string }[] = [
    { dir: 'to-right', label: '→' },
    { dir: 'to-bottom', label: '↓' },
    { dir: 'to-bottom-right', label: '↘' },
    { dir: 'radial', label: '◉' },
  ];

  const zoomPresets: { preset: keyof typeof ZOOM_SPEED_PRESETS; label: string }[] = [
    { preset: 'slow', label: 'Slow' },
    { preset: 'mellow', label: 'Mellow' },
    { preset: 'quick', label: 'Quick' },
    { preset: 'rapid', label: 'Rapid' },
  ];

  return (
    <div className="visual-settings-panel">
      <div className="panel-header">
        <h3>Visual Settings</h3>
        <button
          className="reset-btn"
          onClick={resetVisualSettings}
          title="Reset to defaults"
        >
          <RotateCcw size={14} />
        </button>
      </div>

      {/* Background Section */}
      <div className="settings-section">
        <h4><Palette size={14} /> Background</h4>

        {/* Background Type */}
        <div className="button-group">
          {backgroundTypes.map(({ type, label, icon }) => (
            <button
              key={type}
              className={`type-btn ${visualSettings.backgroundType === type ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ backgroundType: type })}
            >
              {icon}
              <span>{label}</span>
            </button>
          ))}
        </div>

        {/* Solid Color */}
        {visualSettings.backgroundType === 'solid' && (
          <ColorPicker
            label="Color"
            value={visualSettings.backgroundColor}
            onChange={(color) => updateVisualSettings({ backgroundColor: color })}
          />
        )}

        {/* Gradient Controls */}
        {visualSettings.backgroundType === 'gradient' && (
          <>
            <div className="gradient-direction">
              <span className="subsection-label">Direction</span>
              <div className="direction-buttons">
                {gradientDirections.map(({ dir, label }) => (
                  <button
                    key={dir}
                    className={`dir-btn ${visualSettings.gradientDirection === dir ? 'active' : ''}`}
                    onClick={() => updateVisualSettings({ gradientDirection: dir })}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            <div className="gradient-stops">
              <span className="subsection-label">Colors</span>
              {visualSettings.gradientStops.map((stop, i) => (
                <ColorPicker
                  key={i}
                  label={`Stop ${i + 1}`}
                  value={stop.color}
                  onChange={(color) => {
                    const newStops = [...visualSettings.gradientStops];
                    newStops[i] = { ...newStops[i], color };
                    updateVisualSettings({ gradientStops: newStops });
                  }}
                />
              ))}
            </div>
          </>
        )}

        {/* Wallpaper Presets */}
        {visualSettings.backgroundType === 'wallpaper' && (
          <div className="wallpaper-grid">
            {Object.entries(WALLPAPERS).map(([id, wallpaper]) => (
              <button
                key={id}
                className={`wallpaper-btn ${visualSettings.wallpaperId === id ? 'active' : ''}`}
                onClick={() => applyWallpaper(id)}
                style={{
                  background: wallpaper.type === 'gradient'
                    ? `linear-gradient(135deg, ${(wallpaper as any).colors.join(', ')})`
                    : (wallpaper as any).color,
                }}
                title={id.replace('gradient-', '').replace('solid-', '')}
              />
            ))}
          </div>
        )}
      </div>

      {/* Frame Section */}
      <div className="settings-section">
        <h4><Maximize size={14} /> Frame</h4>

        <Slider
          label="Padding"
          value={visualSettings.padding}
          min={0}
          max={50}
          onChange={(padding) => updateVisualSettings({ padding })}
          unit="%"
        />

        <Slider
          label="Corner Radius"
          value={visualSettings.cornerRadius}
          min={0}
          max={50}
          onChange={(cornerRadius) => updateVisualSettings({ cornerRadius })}
          unit="px"
        />

        <Slider
          label="Inset"
          value={visualSettings.inset}
          min={0}
          max={20}
          onChange={(inset) => updateVisualSettings({ inset })}
          unit="px"
        />
      </div>

      {/* Shadow Section */}
      <div className="settings-section">
        <h4><Moon size={14} /> Shadow</h4>

        <label className="toggle-row">
          <input
            type="checkbox"
            checked={visualSettings.shadowEnabled}
            onChange={(e) => updateVisualSettings({ shadowEnabled: e.target.checked })}
          />
          <span>Enable Shadow</span>
        </label>

        {visualSettings.shadowEnabled && (
          <>
            <Slider
              label="Intensity"
              value={visualSettings.shadowIntensity}
              min={0}
              max={100}
              onChange={(shadowIntensity) => updateVisualSettings({ shadowIntensity })}
              unit="%"
            />

            <Slider
              label="Blur"
              value={visualSettings.shadowBlur}
              min={0}
              max={100}
              onChange={(shadowBlur) => updateVisualSettings({ shadowBlur })}
              unit="%"
            />

            <Slider
              label="Offset X"
              value={visualSettings.shadowOffsetX}
              min={-50}
              max={50}
              onChange={(shadowOffsetX) => updateVisualSettings({ shadowOffsetX })}
              unit=""
            />

            <Slider
              label="Offset Y"
              value={visualSettings.shadowOffsetY}
              min={-50}
              max={50}
              onChange={(shadowOffsetY) => updateVisualSettings({ shadowOffsetY })}
              unit=""
            />
          </>
        )}
      </div>

      {/* Cursor Section */}
      <div className="settings-section">
        <h4><MousePointer size={14} /> Cursor</h4>

        <Slider
          label="Size"
          value={visualSettings.cursorScale}
          min={0.5}
          max={3}
          step={0.1}
          onChange={(cursorScale) => updateVisualSettings({ cursorScale })}
          unit="x"
        />

        <Slider
          label="Smoothing"
          value={visualSettings.cursorSmoothing}
          min={0.01}
          max={0.5}
          step={0.01}
          onChange={(cursorSmoothing) => updateVisualSettings({ cursorSmoothing })}
          unit=""
        />

        <div className="subsection-label">Cursor Colors</div>
        <ColorPicker
          label="Cursor"
          value={visualSettings.cursorColor}
          onChange={(cursorColor) => updateVisualSettings({ cursorColor })}
        />
        <ColorPicker
          label="Click Highlight"
          value={visualSettings.cursorHighlightColor}
          onChange={(cursorHighlightColor) => updateVisualSettings({ cursorHighlightColor })}
        />
        <ColorPicker
          label="Ripple"
          value={visualSettings.cursorRippleColor}
          onChange={(cursorRippleColor) => updateVisualSettings({ cursorRippleColor })}
        />

        <Slider
          label="Shadow"
          value={visualSettings.cursorShadowIntensity}
          min={0}
          max={100}
          onChange={(cursorShadowIntensity) => updateVisualSettings({ cursorShadowIntensity })}
          unit="%"
        />

        <label className="toggle-row">
          <input
            type="checkbox"
            checked={visualSettings.cursorTrailEnabled}
            onChange={(e) => updateVisualSettings({ cursorTrailEnabled: e.target.checked })}
          />
          <span>Cursor Trail</span>
        </label>

        {visualSettings.cursorTrailEnabled && (
          <>
            <Slider
              label="Trail Length"
              value={visualSettings.cursorTrailLength}
              min={5}
              max={30}
              onChange={(cursorTrailLength) => updateVisualSettings({ cursorTrailLength })}
              unit=""
            />
            <Slider
              label="Trail Opacity"
              value={visualSettings.cursorTrailOpacity}
              min={0.1}
              max={1}
              step={0.1}
              onChange={(cursorTrailOpacity) => updateVisualSettings({ cursorTrailOpacity })}
              unit=""
            />
          </>
        )}

        <label className="toggle-row">
          <input
            type="checkbox"
            checked={visualSettings.hideCursorWhenIdle}
            onChange={(e) => updateVisualSettings({ hideCursorWhenIdle: e.target.checked })}
          />
          <span>Hide when idle</span>
        </label>

        {visualSettings.hideCursorWhenIdle && (
          <Slider
            label="Idle timeout"
            value={visualSettings.idleTimeout}
            min={500}
            max={5000}
            step={100}
            onChange={(idleTimeout) => updateVisualSettings({ idleTimeout })}
            unit="ms"
          />
        )}
      </div>

      {/* Animation Section */}
      <div className="settings-section">
        <h4><Zap size={14} /> Animation</h4>

        <div className="subsection-label">Zoom Speed</div>
        <div className="button-group zoom-presets">
          {zoomPresets.map(({ preset, label }) => (
            <button
              key={preset}
              className={`preset-btn ${visualSettings.zoomSpeedPreset === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ zoomSpeedPreset: preset })}
            >
              {label}
              <span className="preset-ms">{ZOOM_SPEED_PRESETS[preset].zoomIn}ms</span>
            </button>
          ))}
        </div>

        <div className="subsection-label">Cursor Speed</div>
        <div className="button-group cursor-presets">
          {zoomPresets.map(({ preset, label }) => (
            <button
              key={preset}
              className={`preset-btn ${visualSettings.cursorSpeedPreset === preset ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ cursorSpeedPreset: preset })}
            >
              {label}
            </button>
          ))}
        </div>

        <label className="toggle-row">
          <input
            type="checkbox"
            checked={visualSettings.motionBlurEnabled}
            onChange={(e) => updateVisualSettings({ motionBlurEnabled: e.target.checked })}
          />
          <span>Motion Blur</span>
        </label>

        {visualSettings.motionBlurEnabled && (
          <>
            <Slider
              label="Screen Moving"
              value={visualSettings.motionBlurPanIntensity}
              min={0}
              max={100}
              onChange={(motionBlurPanIntensity) => updateVisualSettings({ motionBlurPanIntensity })}
              unit="%"
            />
            <Slider
              label="Screen Zooming"
              value={visualSettings.motionBlurZoomIntensity}
              min={0}
              max={100}
              onChange={(motionBlurZoomIntensity) => updateVisualSettings({ motionBlurZoomIntensity })}
              unit="%"
            />
            <Slider
              label="Cursor Movement"
              value={visualSettings.motionBlurCursorIntensity}
              min={0}
              max={100}
              onChange={(motionBlurCursorIntensity) => updateVisualSettings({ motionBlurCursorIntensity })}
              unit="%"
            />
          </>
        )}
      </div>

      {/* Aspect Ratio Section */}
      <div className="settings-section">
        <h4><Monitor size={14} /> Output Format</h4>

        <div className="subsection-label">Aspect Ratio</div>
        <div className="aspect-ratio-grid">
          {(Object.entries(ASPECT_RATIOS) as [AspectRatio, typeof ASPECT_RATIOS['16:9']][]).map(([ratio, info]) => (
            <button
              key={ratio}
              className={`aspect-btn ${visualSettings.aspectRatio === ratio ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ aspectRatio: ratio })}
              title={info.description}
            >
              <div
                className="aspect-preview"
                style={{
                  aspectRatio: ratio === 'auto' ? '16/9' : `${info.width}/${info.height}`
                }}
              />
              <span className="aspect-label">{ratio === 'auto' ? 'Auto' : ratio}</span>
              <span className="aspect-desc">{info.label}</span>
            </button>
          ))}
        </div>

        {visualSettings.aspectRatio === '9:16' && (
          <div className="vertical-warning">
            <Smartphone size={14} />
            <span>Perfect for TikTok, Instagram Reels, and YouTube Shorts</span>
          </div>
        )}
      </div>

      {/* Device Frame Section */}
      <div className="settings-section">
        <h4><Smartphone size={14} /> Device Frame</h4>

        <div className="subsection-label">Mockup Style</div>
        <div className="device-frame-grid">
          {(Object.entries(DEVICE_FRAMES) as [DeviceFrame, typeof DEVICE_FRAMES['none']][]).map(([frame, info]) => {
            const Icon = frame === 'none' ? Square :
                         frame.includes('iphone') ? Smartphone :
                         frame.includes('ipad') ? Tablet :
                         frame.includes('macbook') ? Laptop : Globe;
            return (
              <button
                key={frame}
                className={`device-btn ${visualSettings.deviceFrame === frame ? 'active' : ''}`}
                onClick={() => updateVisualSettings({ deviceFrame: frame })}
                title={info.description}
              >
                <Icon size={20} />
                <span className="device-label">{info.label}</span>
              </button>
            );
          })}
        </div>

        {visualSettings.deviceFrame !== 'none' && (
          <>
            <div className="subsection-label">Frame Color</div>
            <div className="button-group frame-colors">
              {['black', 'silver', 'gold', 'blue'].map((color) => (
                <button
                  key={color}
                  className={`color-btn ${visualSettings.deviceFrameColor === color ? 'active' : ''}`}
                  onClick={() => updateVisualSettings({ deviceFrameColor: color })}
                  style={{
                    background: color === 'black' ? '#1a1a1a' :
                               color === 'silver' ? '#c4c4c4' :
                               color === 'gold' ? '#d4af37' : '#2563eb'
                  }}
                  title={color.charAt(0).toUpperCase() + color.slice(1)}
                />
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default VisualSettingsPanel;
