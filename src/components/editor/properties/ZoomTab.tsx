import React, { useState, useRef } from 'react';
import { Zap, Target, Trash2, RotateCcw } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import type { TabProps } from './types';

const SPEED_OPTIONS = ['slow', 'mellow', 'quick', 'rapid'] as const;

const ZoomTab: React.FC<TabProps> = ({ isExporting = false }) => {
  const {
    selectedKeyframe,
    updateZoomKeyframe,
    zoomKeyframes,
    deleteZoomKeyframe,
    zoomAnalysis,
    zoomLoading,
    updateZoomBlock,
    deleteZoomBlock,
    selectedBlockId,
    visualSettings,
  } = useEditorStore();

  // Get the currently selected zoom block
  const selectedBlock = selectedBlockId && zoomAnalysis
    ? zoomAnalysis.zoom_blocks.find(b => b.id === selectedBlockId)
    : null;

  // Calculate constrained center bounds based on zoom factor
  const getConstrainedBounds = (zoomFactor: number) => {
    // At 2x zoom, the visible area is 50% of original
    // Center must be at least (visible_area/2) from edges
    const visibleRatio = 1 / zoomFactor;
    const minCenter = visibleRatio / 2;
    const maxCenter = 1 - (visibleRatio / 2);
    return { min: minCenter * 100, max: maxCenter * 100 };
  };

  // Constrain center value to valid range based on zoom factor
  const constrainCenter = (value: number, zoomFactor: number) => {
    const bounds = getConstrainedBounds(zoomFactor);
    return Math.max(bounds.min, Math.min(bounds.max, value));
  };

  // State for dragging zoom viewport
  const [isDraggingViewport, setIsDraggingViewport] = useState(false);
  const viewportRef = useRef<HTMLDivElement>(null);

  const handleViewportMouseDown = (e: React.MouseEvent) => {
    if (!selectedBlock) return;
    e.preventDefault();
    setIsDraggingViewport(true);
    updateViewportPosition(e);
  };

  const handleViewportMouseMove = (e: React.MouseEvent) => {
    if (!isDraggingViewport || !selectedBlock) return;
    updateViewportPosition(e);
  };

  const handleViewportMouseUp = () => {
    setIsDraggingViewport(false);
  };

  const updateViewportPosition = (e: React.MouseEvent) => {
    if (!viewportRef.current || !selectedBlock) return;

    const rect = viewportRef.current.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = (e.clientY - rect.top) / rect.height;

    // Constrain to valid bounds
    const bounds = getConstrainedBounds(selectedBlock.zoom_factor);
    const constrainedX = Math.max(bounds.min / 100, Math.min(bounds.max / 100, x));
    const constrainedY = Math.max(bounds.min / 100, Math.min(bounds.max / 100, y));

    updateZoomBlock(selectedBlock.id, {
      center_x: constrainedX,
      center_y: constrainedY,
      is_manual: true
    });
  };

  // Calculate zoom window size as percentage of viewport (inverse of zoom factor)
  const zoomWindowSize = selectedBlock ? (1 / selectedBlock.zoom_factor) * 100 : 50;

  return (
    <div className="tab-content">
      {zoomLoading ? (
        <div className="loading-state">
          <div className="editor-spinner" />
          <p>Loading zoom data...</p>
        </div>
      ) : !zoomAnalysis ? (
        <div className="empty-state">
          <Zap size={24} />
          <p>No zoom data available</p>
          <span>Zoom blocks will be created automatically from mouse clicks during recording</span>
        </div>
      ) : (
        <>
          {/* Selected Block Settings (inline) */}
          {selectedBlock ? (
            <div className="section">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <span className={`confidence-badge ${selectedBlock.is_manual ? 'confidence-high' : 'confidence-medium'}`}>
                  {selectedBlock.is_manual ? 'Manual' : 'Auto'}
                </span>
                {selectedBlock.is_manual && (
                  <button
                    className="editor-btn editor-btn--ghost editor-btn--small"
                    onClick={() => updateZoomBlock(selectedBlock.id, {
                      center_x: selectedBlock.click_x,
                      center_y: selectedBlock.click_y,
                      is_manual: false
                    })}
                    title="Reset center to original click position"
                    disabled={isExporting}
                  >
                    <RotateCcw size={12} />
                    Reset
                  </button>
                )}
              </div>

              {/* Zoom Factor Slider */}
              <div className="control-group">
                <label>Zoom Factor</label>
                <div className="slider-control">
                  <input
                    type="range"
                    min="1.2"
                    max="4"
                    step="0.1"
                    value={selectedBlock.zoom_factor}
                    onChange={(e) => {
                      const newZoomFactor = parseFloat(e.target.value);
                      // Auto-constrain center when zoom factor changes
                      updateZoomBlock(selectedBlock.id, {
                        zoom_factor: newZoomFactor,
                        center_x: constrainCenter(selectedBlock.center_x * 100, newZoomFactor) / 100,
                        center_y: constrainCenter(selectedBlock.center_y * 100, newZoomFactor) / 100,
                      });
                    }}
                    className="editor-slider"
                    disabled={isExporting}
                  />
                  <span className="value-display">{selectedBlock.zoom_factor.toFixed(1)}x</span>
                </div>
              </div>

              {/* Block Type Badge */}
              {selectedBlock.kind && (
                <div className="control-group">
                  <label>Block Type</label>
                  <span className={`confidence-badge ${selectedBlock.kind === 'typing' ? 'confidence-high' : 'confidence-medium'}`}
                    style={{ textTransform: 'capitalize' }}>
                    {selectedBlock.kind}
                  </span>
                </div>
              )}

              {/* Per-Block Zoom-In Speed */}
              <div className="control-group">
                <label>Zoom-In Speed</label>
                <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '4px' }}>
                  {SPEED_OPTIONS.map((speed) => (
                    <button
                      key={speed}
                      className={`editor-btn editor-btn--small ${selectedBlock.zoom_in_speed === speed ? 'editor-btn--primary' : 'editor-btn--ghost'}`}
                      onClick={() => updateZoomBlock(selectedBlock.id, {
                        zoom_in_speed: selectedBlock.zoom_in_speed === speed ? undefined : speed,
                      })}
                      disabled={isExporting}
                      style={{ textTransform: 'capitalize', fontSize: '11px', padding: '3px 8px' }}
                    >
                      {speed}
                    </button>
                  ))}
                </div>
                <small className="setting-hint" style={{ marginTop: '4px', display: 'block' }}>
                  {selectedBlock.zoom_in_speed
                    ? `Override: ${selectedBlock.zoom_in_speed}`
                    : `Using global: ${visualSettings.zoomSpeedPreset}`}
                </small>
              </div>

              {/* Per-Block Zoom-Out Speed */}
              <div className="control-group">
                <label>Zoom-Out Speed</label>
                <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '4px' }}>
                  {SPEED_OPTIONS.map((speed) => (
                    <button
                      key={speed}
                      className={`editor-btn editor-btn--small ${selectedBlock.zoom_out_speed === speed ? 'editor-btn--primary' : 'editor-btn--ghost'}`}
                      onClick={() => updateZoomBlock(selectedBlock.id, {
                        zoom_out_speed: selectedBlock.zoom_out_speed === speed ? undefined : speed,
                      })}
                      disabled={isExporting}
                      style={{ textTransform: 'capitalize', fontSize: '11px', padding: '3px 8px' }}
                    >
                      {speed}
                    </button>
                  ))}
                </div>
                <small className="setting-hint" style={{ marginTop: '4px', display: 'block' }}>
                  {selectedBlock.zoom_out_speed
                    ? `Override: ${selectedBlock.zoom_out_speed}`
                    : `Using global: ${visualSettings.zoomSpeedPreset}`}
                </small>
              </div>

              {/* Visual Viewport Position Picker */}
              <div className="control-group">
                <label>Zoom Position</label>
                <div
                  ref={viewportRef}
                  className="zoom-viewport-picker"
                  onMouseDown={isExporting ? undefined : handleViewportMouseDown}
                  onMouseMove={isExporting ? undefined : handleViewportMouseMove}
                  onMouseUp={isExporting ? undefined : handleViewportMouseUp}
                  onMouseLeave={isExporting ? undefined : handleViewportMouseUp}
                  style={{
                    position: 'relative',
                    width: '100%',
                    aspectRatio: '16/9',
                    background: 'var(--editor-bg-tertiary)',
                    borderRadius: '6px',
                    border: '1px solid var(--editor-border)',
                    cursor: isExporting ? 'not-allowed' : 'crosshair',
                    overflow: 'hidden',
                    marginTop: '8px',
                    opacity: isExporting ? 0.5 : 1
                  }}
                >
                  {/* Grid lines for reference */}
                  <div style={{
                    position: 'absolute',
                    inset: 0,
                    backgroundImage: 'linear-gradient(var(--editor-border) 1px, transparent 1px), linear-gradient(90deg, var(--editor-border) 1px, transparent 1px)',
                    backgroundSize: '25% 25%',
                    opacity: 0.3
                  }} />

                  {/* Original click position marker */}
                  <div
                    style={{
                      position: 'absolute',
                      left: `${selectedBlock.click_x * 100}%`,
                      top: `${selectedBlock.click_y * 100}%`,
                      width: '8px',
                      height: '8px',
                      background: 'var(--dracula-red)',
                      borderRadius: '50%',
                      transform: 'translate(-50%, -50%)',
                      opacity: 0.6,
                      pointerEvents: 'none'
                    }}
                    title="Original click position"
                  />

                  {/* Zoom window indicator */}
                  <div
                    style={{
                      position: 'absolute',
                      left: `${selectedBlock.center_x * 100}%`,
                      top: `${selectedBlock.center_y * 100}%`,
                      width: `${zoomWindowSize}%`,
                      height: `${zoomWindowSize}%`,
                      border: '2px solid var(--accent-primary)',
                      borderRadius: '4px',
                      transform: 'translate(-50%, -50%)',
                      background: 'rgba(189, 147, 249, 0.1)',
                      boxShadow: '0 0 0 2000px rgba(0,0,0,0.4)',
                      pointerEvents: 'none'
                    }}
                  />

                  {/* Center crosshair */}
                  <div
                    style={{
                      position: 'absolute',
                      left: `${selectedBlock.center_x * 100}%`,
                      top: `${selectedBlock.center_y * 100}%`,
                      width: '12px',
                      height: '12px',
                      transform: 'translate(-50%, -50%)',
                      pointerEvents: 'none'
                    }}
                  >
                    <div style={{ position: 'absolute', left: '50%', top: 0, width: '2px', height: '100%', background: 'var(--accent-primary)', transform: 'translateX(-50%)' }} />
                    <div style={{ position: 'absolute', top: '50%', left: 0, width: '100%', height: '2px', background: 'var(--accent-primary)', transform: 'translateY(-50%)' }} />
                  </div>
                </div>
                <small className="setting-hint" style={{ marginTop: '6px', display: 'block' }}>
                  Click or drag to set zoom center
                </small>
              </div>

              {/* Delete Action */}
              <div className="block-actions" style={{ marginTop: '16px' }}>
                <button
                  className="editor-btn editor-btn--danger"
                  onClick={() => {
                    deleteZoomBlock(selectedBlock.id);
                  }}
                  disabled={isExporting}
                >
                  <Trash2 size={14} />
                  Delete Block
                </button>
              </div>
            </div>
          ) : (
            <div className="section">
              <div className="empty-state" style={{ padding: '24px 16px' }}>
                <Target size={24} />
                <p>No block selected</p>
                <span>
                  {zoomAnalysis.zoom_blocks.length} zoom block{zoomAnalysis.zoom_blocks.length !== 1 ? 's' : ''} available.
                  Click one in the timeline to edit.
                </span>
              </div>
            </div>
          )}
        </>
      )}

      {/* Manual Zoom Keyframes */}
      {zoomKeyframes.length > 0 && (
        <div className="section">
          <div className="section-header">
            <h3>Manual Zoom Keyframes</h3>
            <p>Custom zoom keyframes you've added</p>
          </div>

          <div className="zoom-clusters">
            {zoomKeyframes.map((keyframe, index) => (
              <div key={keyframe.id} className="cluster-item">
                <div className="cluster-info">
                  <div className="cluster-name">Manual Zoom {index + 1}</div>
                  <div className="cluster-details">
                    {keyframe.scale}x zoom at {Math.round(keyframe.time / 1000)}s
                  </div>
                </div>
                <div className="cluster-controls">
                  <button
                    className="editor-btn editor-btn--ghost editor-btn--small"
                    title="Delete keyframe"
                    onClick={() => deleteZoomKeyframe(keyframe.id)}
                    disabled={isExporting}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {selectedKeyframe && (
        <div className="section">
          <div className="section-header">
            <h3>Keyframe Settings</h3>
          </div>

          <div className="control-group">
            <label>Zoom Level</label>
            <div className="slider-control">
              <input
                type="range"
                min="1"
                max="3"
                step="0.1"
                value={selectedKeyframe.scale}
                onChange={(e) => updateZoomKeyframe(selectedKeyframe.id, {
                  scale: parseFloat(e.target.value)
                })}
                className="editor-slider"
                disabled={isExporting}
              />
              <span className="value-display">{selectedKeyframe.scale.toFixed(1)}x</span>
            </div>
          </div>

          <div className="control-group">
            <label>Duration</label>
            <div className="slider-control">
              <input
                type="range"
                min="100"
                max="2000"
                step="50"
                value={selectedKeyframe.duration}
                onChange={(e) => updateZoomKeyframe(selectedKeyframe.id, {
                  duration: parseInt(e.target.value)
                })}
                className="editor-slider"
                disabled={isExporting}
              />
              <span className="value-display">{selectedKeyframe.duration}ms</span>
            </div>
          </div>

          <div className="control-group">
            <label>Center X</label>
            <div className="slider-control">
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={selectedKeyframe.centerX}
                onChange={(e) => updateZoomKeyframe(selectedKeyframe.id, {
                  centerX: parseFloat(e.target.value)
                })}
                className="editor-slider"
                disabled={isExporting}
              />
              <span className="value-display">{Math.round(selectedKeyframe.centerX * 100)}%</span>
            </div>
          </div>

          <div className="control-group">
            <label>Center Y</label>
            <div className="slider-control">
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={selectedKeyframe.centerY}
                onChange={(e) => updateZoomKeyframe(selectedKeyframe.id, {
                  centerY: parseFloat(e.target.value)
                })}
                className="editor-slider"
                disabled={isExporting}
              />
              <span className="value-display">{Math.round(selectedKeyframe.centerY * 100)}%</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ZoomTab;
