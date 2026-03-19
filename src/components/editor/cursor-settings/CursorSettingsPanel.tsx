import React from 'react';
import { useEditorStore } from '../../../stores/editor';
import { StyleSettings } from './StyleSettings';
import { BehaviorSettings } from './BehaviorSettings';

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
  const { visualSettings, updateVisualSettings } = useEditorStore();

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

      <style>{`
        .cursor-settings-panel {
          display: flex;
          flex-direction: column;
          gap: 0;
          min-width: 0;
          max-width: 100%;
          overflow: hidden;
        }

        .cursor-setting-section {
          padding: 12px 0;
          min-width: 0;
          max-width: 100%;
          overflow: hidden;
        }

        .cursor-setting-label {
          font-size: 14px;
          font-weight: 500;
          color: var(--editor-text-primary);
          display: block;
          margin-bottom: 8px;
        }

        .cursor-divider {
          height: 1px;
          background: var(--editor-border);
          margin: 0;
        }

        /* Slider with reset */
        .slider-with-reset {
          display: flex;
          align-items: center;
          gap: 12px;
          min-width: 0;
          max-width: 100%;
          overflow: hidden;
        }

        .reset-btn {
          padding: 6px 14px;
          background: transparent;
          border: none;
          color: var(--editor-text-secondary);
          cursor: pointer;
          font-size: 13px;
          font-weight: 500;
          border-radius: 6px;
          transition: all 0.15s ease;
        }

        .reset-btn:hover {
          color: var(--editor-text-primary);
          background: var(--editor-bg-tertiary);
        }

        /* Cursor style scroll container with fades */
        .cursor-style-scroll-container {
          position: relative;
          min-width: 0;
          max-width: 100%;
        }

        .cursor-style-scroll-container::before,
        .cursor-style-scroll-container::after {
          content: '';
          position: absolute;
          top: 0;
          bottom: 0;
          width: 24px;
          pointer-events: none;
          z-index: 1;
          opacity: 0;
          transition: opacity 0.15s ease;
        }

        .cursor-style-scroll-container::before {
          left: 0;
          background: linear-gradient(to right, var(--editor-bg-primary, #1a1a1a) 0%, transparent 100%);
        }

        .cursor-style-scroll-container::after {
          right: 0;
          background: linear-gradient(to left, var(--editor-bg-primary, #1a1a1a) 0%, transparent 100%);
        }

        .cursor-style-scroll-container.show-left-fade::before {
          opacity: 1;
        }

        .cursor-style-scroll-container.show-right-fade::after {
          opacity: 1;
        }

        /* Cursor style grid */
        .cursor-style-grid {
          display: flex;
          gap: 8px;
          min-width: 0;
          overflow-x: auto;
          scrollbar-width: none;
          -ms-overflow-style: none;
          padding: 2px 0;
        }

        .cursor-style-grid::-webkit-scrollbar {
          display: none;
        }

        .cursor-style-btn {
          width: 48px;
          height: 48px;
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--editor-bg-secondary);
          border: 2px solid var(--editor-border);
          border-radius: 10px;
          cursor: pointer;
          transition: all 0.15s ease;
          padding: 8px;
        }

        .cursor-style-btn:hover {
          background: var(--editor-bg-tertiary);
          border-color: rgba(139, 92, 246, 0.5);
        }

        .cursor-style-btn.active {
          border-color: #8B5CF6;
          background: rgba(139, 92, 246, 0.15);
        }

        /* Toggle row */
        .toggle-row {
          display: flex;
          justify-content: space-between;
          align-items: flex-start;
          gap: 16px;
          min-width: 0;
          max-width: 100%;
        }

        .toggle-info {
          flex: 1;
          min-width: 0;
          max-width: calc(100% - 60px);
          overflow: hidden;
        }

        .toggle-info .cursor-setting-label {
          margin-bottom: 4px;
        }

        .toggle-description {
          font-size: 12px;
          color: var(--editor-text-secondary);
          line-height: 1.4;
          display: block;
          word-wrap: break-word;
          overflow-wrap: break-word;
        }

        .toggle-label-with-icon {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 4px;
        }

        .toggle-label-with-icon .cursor-setting-label {
          margin-bottom: 0;
        }

        .toggle-label-with-icon svg {
          color: var(--editor-text-secondary);
        }

        /* Toggle switch */
        .toggle-switch {
          position: relative;
          display: inline-block;
          width: 44px;
          height: 24px;
          flex-shrink: 0;
        }

        .toggle-switch input {
          opacity: 0;
          width: 0;
          height: 0;
        }

        .toggle-slider {
          position: absolute;
          cursor: pointer;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background-color: var(--editor-bg-tertiary);
          border-radius: 24px;
          transition: 0.2s;
        }

        .toggle-slider:before {
          position: absolute;
          content: "";
          height: 18px;
          width: 18px;
          left: 3px;
          bottom: 3px;
          background-color: var(--editor-text-secondary);
          border-radius: 50%;
          transition: 0.2s;
        }

        .toggle-switch input:checked + .toggle-slider {
          background-color: #8B5CF6;
        }

        .toggle-switch input:checked + .toggle-slider:before {
          transform: translateX(20px);
          background-color: white;
        }

        /* Click effect group */
        .click-effect-group {
          display: flex;
          gap: 8px;
          min-width: 0;
          max-width: 100%;
          overflow: hidden;
        }

        .click-effect-btn {
          flex: 1;
          min-width: 0;
          padding: 10px 12px;
          background: var(--editor-bg-secondary);
          border: 1px solid var(--editor-border);
          border-radius: 8px;
          color: var(--editor-text-primary);
          font-size: 13px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.15s ease;
        }

        .click-effect-btn:hover {
          background: var(--editor-bg-tertiary);
          border-color: rgba(139, 92, 246, 0.5);
        }

        .click-effect-btn.active {
          background: rgba(139, 92, 246, 0.15);
          border-color: #8B5CF6;
          color: #8B5CF6;
        }

        /* Collapsible section */
        .collapsible-section {
          border-top: 1px solid var(--editor-border);
          min-width: 0;
          max-width: 100%;
          overflow: hidden;
        }

        .collapsible-section .section-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          width: 100%;
          padding: 14px 0;
          background: transparent;
          border: none;
          cursor: pointer;
          color: var(--editor-text-primary);
        }

        .section-title {
          font-size: 14px;
          font-weight: 600;
        }

        .chevron {
          color: var(--editor-text-secondary);
          transition: transform 0.2s ease;
          transform: rotate(180deg);
        }

        .chevron.expanded {
          transform: rotate(0deg);
        }

        .section-content {
          padding-bottom: 12px;
          min-width: 0;
          overflow: hidden;
        }

        .section-content .cursor-setting-section {
          padding: 8px 0;
        }

        .section-content .cursor-setting-section:first-child {
          padding-top: 0;
        }

        /* Setting description (longer text) */
        .setting-description {
          font-size: 12px;
          color: var(--editor-text-secondary);
          line-height: 1.5;
          display: block;
          margin-top: 4px;
          word-wrap: break-word;
          overflow-wrap: break-word;
        }
      `}</style>
    </div>
  );
};

export default CursorSettingsPanel;
