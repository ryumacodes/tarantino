import React, { useRef, useState, useEffect, useCallback } from 'react';
import { EyeOff } from 'lucide-react';
import { CursorStylePreview, type CursorStyle } from './CursorStylePreview';
import type { VisualSettings } from '../../../stores/editor';

interface StyleSettingsProps {
  visualSettings: VisualSettings;
  updateVisualSettings: (settings: Partial<VisualSettings>) => void;
  isExporting?: boolean;
}

const cursorStyles: { id: CursorStyle; label: string }[] = [
  { id: 'pointer', label: 'Pointer' },
  { id: 'circle', label: 'Circle' },
  { id: 'filled', label: 'Filled' },
  { id: 'outline', label: 'Outline' },
  { id: 'dotted', label: 'Dotted' },
];

const clickEffects = [
  { id: 'none' as const, label: 'None' },
  { id: 'circle' as const, label: 'Circle' },
  { id: 'ripple' as const, label: 'Ripple' },
];

export const StyleSettings: React.FC<StyleSettingsProps> = ({
  visualSettings,
  updateVisualSettings,
  isExporting = false
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [showLeftFade, setShowLeftFade] = useState(false);
  const [showRightFade, setShowRightFade] = useState(false);

  const checkScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;

    const { scrollLeft, scrollWidth, clientWidth } = el;
    setShowLeftFade(scrollLeft > 0);
    setShowRightFade(scrollLeft < scrollWidth - clientWidth - 1);
  }, []);

  useEffect(() => {
    checkScroll();
    const el = scrollRef.current;
    if (el) {
      el.addEventListener('scroll', checkScroll);
      // Also check on resize
      const resizeObserver = new ResizeObserver(checkScroll);
      resizeObserver.observe(el);
      return () => {
        el.removeEventListener('scroll', checkScroll);
        resizeObserver.disconnect();
      };
    }
  }, [checkScroll]);

  return (
    <>
      {/* Cursor Size */}
      <div className="cursor-setting-section">
        <label className="cursor-setting-label">Cursor size</label>
        <div className="slider-with-reset">
          <input
            type="range"
            min="1"
            max="5"
            step="0.1"
            value={visualSettings.cursorScale}
            onChange={(e) => updateVisualSettings({ cursorScale: parseFloat(e.target.value) })}
            className="editor-slider"
            disabled={isExporting}
            data-testid="cursor-size-slider"
          />
          <button
            className="reset-btn"
            onClick={() => updateVisualSettings({ cursorScale: 3.0 })}
            disabled={isExporting}
          >
            Reset
          </button>
        </div>
      </div>

      <div className="cursor-divider" />

      {/* Cursor Style */}
      <div className="cursor-setting-section">
        <label className="cursor-setting-label">Cursor style</label>
        <div className={`cursor-style-scroll-container ${showLeftFade ? 'show-left-fade' : ''} ${showRightFade ? 'show-right-fade' : ''}`}>
          <div className="cursor-style-grid" ref={scrollRef}>
            {cursorStyles.map((style) => (
              <button
                key={style.id}
                className={`cursor-style-btn ${visualSettings.cursorStyle === style.id ? 'active' : ''}`}
                onClick={() => updateVisualSettings({ cursorStyle: style.id })}
                title={style.label}
                disabled={isExporting}
                data-testid={`cursor-style-${style.id}`}
              >
                <CursorStylePreview style={style.id} />
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="cursor-divider" />

      {/* Always use pointer cursor */}
      <div className="cursor-setting-section">
        <div className="toggle-row">
          <div className="toggle-info">
            <label className="cursor-setting-label">Always use pointer cursor</label>
            <span className="toggle-description">
              Don't change cursor, even if selecting text, etc.
            </span>
          </div>
          <label className="toggle-switch">
            <input
              type="checkbox"
              checked={visualSettings.alwaysUsePointer}
              onChange={(e) => updateVisualSettings({ alwaysUsePointer: e.target.checked })}
              disabled={isExporting}
              data-testid="always-pointer-toggle"
            />
            <span className="toggle-slider" />
          </label>
        </div>
      </div>

      {/* Hide cursor if not moving */}
      <div className="cursor-setting-section">
        <div className="toggle-row">
          <div className="toggle-info">
            <label className="cursor-setting-label">Hide cursor if not moving</label>
          </div>
          <label className="toggle-switch">
            <input
              type="checkbox"
              checked={visualSettings.hideCursorWhenIdle}
              onChange={(e) => updateVisualSettings({ hideCursorWhenIdle: e.target.checked })}
              disabled={isExporting}
            />
            <span className="toggle-slider" />
          </label>
        </div>
      </div>

      <div className="cursor-divider" />

      {/* Loop cursor position */}
      <div className="cursor-setting-section">
        <div className="toggle-row">
          <div className="toggle-info">
            <label className="cursor-setting-label">Loop cursor position</label>
            <span className="toggle-description">
              Near the end of the video, cursor will move back to its initial position
            </span>
          </div>
          <label className="toggle-switch">
            <input
              type="checkbox"
              checked={visualSettings.loopCursorPosition}
              onChange={(e) => updateVisualSettings({ loopCursorPosition: e.target.checked })}
              disabled={isExporting}
            />
            <span className="toggle-slider" />
          </label>
        </div>
      </div>

      {/* Hide cursor */}
      <div className="cursor-setting-section">
        <div className="toggle-row">
          <div className="toggle-info">
            <div className="toggle-label-with-icon">
              <EyeOff size={16} />
              <label className="cursor-setting-label">Hide cursor</label>
            </div>
          </div>
          <label className="toggle-switch">
            <input
              type="checkbox"
              checked={visualSettings.hideCursor}
              onChange={(e) => updateVisualSettings({ hideCursor: e.target.checked })}
              disabled={isExporting}
              data-testid="hide-cursor-toggle"
            />
            <span className="toggle-slider" />
          </label>
        </div>
      </div>

      <div className="cursor-divider" />

      {/* Click effect */}
      <div className="cursor-setting-section">
        <label className="cursor-setting-label">Click effect</label>
        <div className="click-effect-group">
          {clickEffects.map((effect) => (
            <button
              key={effect.id}
              className={`click-effect-btn ${visualSettings.clickEffect === effect.id ? 'active' : ''}`}
              onClick={() => updateVisualSettings({ clickEffect: effect.id })}
              disabled={isExporting}
              data-testid={`click-effect-${effect.id}`}
            >
              {effect.label}
            </button>
          ))}
        </div>
      </div>
    </>
  );
};

export default StyleSettings;
