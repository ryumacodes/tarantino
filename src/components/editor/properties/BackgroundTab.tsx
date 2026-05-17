import React, { useRef } from 'react';
import { Image, Layers, Monitor, Palette } from 'lucide-react';
import { useEditorStore, ASPECT_RATIOS, WALLPAPERS, getWallpaperBackground, type AspectRatio, type BackgroundType, type GradientDirection } from '../../../stores/editor';
import type { TabProps } from './types';

const backgroundTypes: { type: BackgroundType; label: string; icon: React.ReactNode }[] = [
  { type: 'solid', label: 'Solid', icon: <Monitor size={16} /> },
  { type: 'gradient', label: 'Gradient', icon: <Palette size={16} /> },
  { type: 'wallpaper', label: 'Wallpaper', icon: <Layers size={16} /> },
];

const gradientDirections: { dir: GradientDirection; label: string }[] = [
  { dir: 'to-right', label: '→' },
  { dir: 'to-bottom', label: '↓' },
  { dir: 'to-bottom-right', label: '↘' },
  { dir: 'radial', label: '◉' },
];

const SUPPORTED_WALLPAPER_IMAGE_TYPES = new Set([
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/gif',
  'image/avif',
  'image/bmp',
]);

const SUPPORTED_WALLPAPER_IMAGE_EXTENSIONS = /\.(png|jpe?g|webp|gif|avif|bmp)$/i;

const isSupportedWallpaperImage = (file: File) =>
  SUPPORTED_WALLPAPER_IMAGE_TYPES.has(file.type) || SUPPORTED_WALLPAPER_IMAGE_EXTENSIONS.test(file.name);

const readWallpaperImage = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onload = () => {
    if (typeof reader.result === 'string') {
      resolve(reader.result);
    } else {
      reject(new Error('Invalid image data'));
    }
  };
  reader.onerror = () => {
    console.error('[Wallpaper Image] file read failed', reader.error);
    reject(reader.error ?? new Error('Unable to read image'));
  };
  reader.readAsDataURL(file);
});

const BackgroundTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const { visualSettings, updateVisualSettings, applyWallpaper, applyCustomWallpaper, captureMode } = useEditorStore();
  const imageInputRef = useRef<HTMLInputElement>(null);
  const isWindowFocus = captureMode === 'window' && visualSettings.windowLayoutMode === 'focus';
  const showAspectRatio = captureMode !== 'window' || visualSettings.windowLayoutMode === 'desktop';

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
              onClick={() => {
                if (type === 'wallpaper') {
                  if (visualSettings.customBackgroundImage && !visualSettings.wallpaperId) {
                    updateVisualSettings({ backgroundType: 'wallpaper' });
                  } else {
                    applyWallpaper(visualSettings.wallpaperId ?? 'gradient-purple');
                  }
                } else {
                  updateVisualSettings({ backgroundType: type, wallpaperId: null, customBackgroundImage: null });
                }
              }}
              disabled={isExporting}
            >
              {icon}
              <span>{label}</span>
            </button>
          ))}
        </div>
      </div>

      {captureMode === 'window' && (
        <div className="section">
          <div className="section-header">
            <h3>Window Layout</h3>
          </div>
          <div className="quality-presets">
            <button
              className={`quality-preset ${isWindowFocus ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ windowLayoutMode: 'focus', aspectRatio: 'auto' })}
              disabled={isExporting}
            >
              <span>Focus</span>
              <small>Original window</small>
            </button>
            <button
              className={`quality-preset ${visualSettings.windowLayoutMode === 'desktop' ? 'active' : ''}`}
              onClick={() => updateVisualSettings({
                windowLayoutMode: 'desktop',
                aspectRatio: visualSettings.aspectRatio === 'auto' ? '16:9' : visualSettings.aspectRatio,
              })}
              disabled={isExporting}
            >
              <span>Desktop</span>
              <small>Wallpaper stage</small>
            </button>
          </div>
        </div>
      )}

      {showAspectRatio && (
      <div className="section">
        <div className="section-header">
          <h3>Aspect Ratio</h3>
          <p>Choose the editor and export canvas</p>
        </div>
        <div className="aspect-ratio-grid">
          {(Object.entries(ASPECT_RATIOS) as [AspectRatio, typeof ASPECT_RATIOS[keyof typeof ASPECT_RATIOS]][]).map(([ratio, info]) => (
            <button
              key={ratio}
              className={`aspect-ratio-btn ${visualSettings.aspectRatio === ratio ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ aspectRatio: ratio })}
              disabled={isExporting}
            >
              <div className="aspect-preview" style={{ aspectRatio: ratio === 'auto' ? '16/9' : `${info.width}/${info.height}` }}>
                <span className="aspect-value">{ratio === 'auto' ? 'Auto' : ratio}</span>
              </div>
              <span className="aspect-label">{info.label}</span>
            </button>
          ))}
        </div>
      </div>
      )}

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
            <h3>Wallpapers</h3>
          </div>
          <div className="control-group">
            <button
              className={`quality-preset ${visualSettings.customBackgroundImage && !visualSettings.wallpaperId ? 'active' : ''}`}
              onClick={() => imageInputRef.current?.click()}
              disabled={isExporting}
            >
              <Image size={16} />
              <span>{visualSettings.customBackgroundImage ? 'Custom Image' : 'Choose Image'}</span>
            </button>
            <input
              ref={imageInputRef}
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif,image/avif,image/bmp"
              style={{ display: 'none' }}
              onChange={async (event) => {
                const file = event.target.files?.[0];
                if (!file) return;
                try {
                  if (!isSupportedWallpaperImage(file)) {
                    window.alert('Choose a JPG, PNG, WebP, GIF, AVIF, or BMP image.');
                    return;
                  }
                  const imageDataUrl = await readWallpaperImage(file);
                  applyCustomWallpaper(imageDataUrl);
                } catch (error) {
                  console.error('Failed to load wallpaper image:', error);
                  window.alert('That image could not be loaded. Try a JPG or PNG.');
                } finally {
                  event.target.value = '';
                }
              }}
            />
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
                    background: getWallpaperBackground(wallpaper),
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
