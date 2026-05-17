import React from 'react';
import {
  ZoomIn,
  ZoomOut,
  Maximize2,
  RotateCcw,
  MousePointer,
  Scissors,
  Move,
  MoreHorizontal,
  Hand,
  Search,
  Plus
} from 'lucide-react';
import type { TimelineTool } from '../../../stores/editor/types';

interface TimelineHeaderProps {
  currentTime: number;
  duration: number;
  currentTool: TimelineTool;
  setCurrentTool: (tool: TimelineTool) => void;
  snappingEnabled: boolean;
  setSnappingEnabled: (enabled: boolean) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFitTimeline: () => void;
  onToggleCollapse?: () => void;
  onAddZoomBlock: () => void;
  isExporting: boolean;
}

export const formatTime = (ms: number): string => {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  const remainingMs = Math.floor((ms % 1000) / 10);
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}.${remainingMs.toString().padStart(2, '0')}`;
};

const TimelineHeader: React.FC<TimelineHeaderProps> = ({
  currentTime,
  duration,
  currentTool,
  setCurrentTool,
  snappingEnabled,
  setSnappingEnabled,
  onZoomIn,
  onZoomOut,
  onFitTimeline,
  onToggleCollapse,
  onAddZoomBlock,
  isExporting,
}) => {
  return (
    <div className="timeline-header">
      <div className="timeline-info">
        <span className="current-time">{formatTime(currentTime)}</span>
        <span className="time-separator">/</span>
        <span className="total-time">{formatTime(duration)}</span>
      </div>

      <div className="timeline-toolbar">
        {/* Tool Selection */}
        <div className="tool-group">
          <button
            className={`tool-btn ${currentTool === 'select' ? 'active' : ''}`}
            onClick={() => setCurrentTool('select')}
            title="Selection Tool (V)"
          >
            <MousePointer size={16} />
          </button>
          <button
            className={`tool-btn ${currentTool === 'scissors' ? 'active' : ''} ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && setCurrentTool('scissors')}
            title={isExporting ? "Disabled during export" : "Scissors Tool (C)"}
            disabled={isExporting}
          >
            <Scissors size={16} />
          </button>
          <button
            className={`tool-btn ${currentTool === 'trim' ? 'active' : ''} ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && setCurrentTool('trim')}
            title={isExporting ? "Disabled during export" : "Trim Tool (T)"}
            disabled={isExporting}
          >
            <MoreHorizontal size={16} />
          </button>
          <button
            className={`tool-btn ${currentTool === 'slip' ? 'active' : ''} ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && setCurrentTool('slip')}
            title={isExporting ? "Disabled during export" : "Slip Tool (S)"}
            disabled={isExporting}
          >
            <Move size={16} />
          </button>
          <button
            className={`tool-btn ${currentTool === 'pan' ? 'active' : ''} ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && setCurrentTool('pan')}
            title={isExporting ? "Disabled during export" : "Pan Tool (H)"}
            disabled={isExporting}
          >
            <Hand size={16} />
          </button>
        </div>

        {/* Add Zoom Block */}
        <div className="tool-group">
          <button
            className={`tool-btn ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && onAddZoomBlock()}
            title={isExporting ? "Disabled during export" : "Add Zoom Block"}
            disabled={isExporting}
          >
            <Plus size={16} />
          </button>
        </div>

        {/* Snapping Toggle */}
        <div className="tool-group">
          <button
            className={`tool-btn ${snappingEnabled ? 'active' : ''} ${isExporting ? 'disabled' : ''}`}
            onClick={() => !isExporting && setSnappingEnabled(!snappingEnabled)}
            title={isExporting ? "Disabled during export" : "Toggle Snapping (N)"}
            disabled={isExporting}
          >
            <Search size={16} />
          </button>
        </div>

        {/* Zoom Controls */}
        <div className="tool-group">
          <button
            className="tool-btn"
            onClick={onZoomOut}
            title="Zoom Out (-)"
          >
            <ZoomOut size={14} />
          </button>
          <button
            className="tool-btn"
            onClick={onFitTimeline}
            title="Fit Timeline (0)"
          >
            <RotateCcw size={14} />
          </button>
          <button
            className="tool-btn"
            onClick={onZoomIn}
            title="Zoom In (+)"
          >
            <ZoomIn size={14} />
          </button>
        </div>

        {/* Collapse Toggle */}
        {onToggleCollapse && (
          <div className="tool-group">
            <button
              className="tool-btn"
              onClick={onToggleCollapse}
              title="Collapse Timeline"
            >
              <Maximize2 size={14} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default TimelineHeader;
