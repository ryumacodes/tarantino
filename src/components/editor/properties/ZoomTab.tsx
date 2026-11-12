import { useShallow } from 'zustand/react/shallow';
import React, { useState, useRef, useMemo } from 'react';
import { Zap, Target, Trash2, RotateCcw } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import type { TabProps } from './types';
import { ZoomPresentationControls } from './ZoomPresentationControls';
import { RebuildZooms } from './RebuildZooms';
import { manualZoomCenterUpdate, zoomCenterBounds, zoomPickerCenter } from '../../../utils/motion/zoomCenterBounds';
import { cropSourcePoint, resolveSourceCrop } from '../../../utils/sourceCrop';

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
    updateVisualSettings,
    captureMode,
    videoWidth,
    videoHeight,
  } = useEditorStore(useShallow(state => ({
    selectedKeyframe: state.selectedKeyframe,
    updateZoomKeyframe: state.updateZoomKeyframe,
    zoomKeyframes: state.zoomKeyframes,
    deleteZoomKeyframe: state.deleteZoomKeyframe,
    zoomAnalysis: state.zoomAnalysis,
    zoomLoading: state.zoomLoading,
    updateZoomBlock: state.updateZoomBlock,
    deleteZoomBlock: state.deleteZoomBlock,
    selectedBlockId: state.selectedBlockId,
    visualSettings: state.visualSettings,
    updateVisualSettings: state.updateVisualSettings,
    captureMode: state.captureMode,
    videoWidth: state.videoWidth,
    videoHeight: state.videoHeight,
  })));

  // Get the currently selected zoom block
  const selectedBlock = selectedBlockId && zoomAnalysis
    ? zoomAnalysis.zoom_blocks.find(b => b.id === selectedBlockId)
    : null;

  const crop = useMemo(() => visualSettings.sourceCrop && videoWidth && videoHeight
    ? resolveSourceCrop(visualSettings.sourceCrop, videoWidth, videoHeight) : null,
  [visualSettings.sourceCrop, videoWidth, videoHeight]);
  const pickerCenter = selectedBlock ? zoomPickerCenter(selectedBlock, crop) : { x: 0.5, y: 0.5 };
  const clickPosition = selectedBlock && crop
    ? cropSourcePoint(crop, selectedBlock.click_x, selectedBlock.click_y)
    : { x: selectedBlock?.click_x ?? 0.5, y: selectedBlock?.click_y ?? 0.5 };
  const pickerAspect = crop ? crop.width / crop.height : videoWidth && videoHeight ? videoWidth / videoHeight : 16 / 9;

  const getConstrainedBounds = (zoomFactor: number) => {
    const bounds = zoomCenterBounds(zoomFactor, captureMode, !!selectedBlock?.loupe);
    return { min: bounds.min * 100, max: bounds.max * 100 };
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
      <RebuildZooms disabled={isExporting || zoomLoading} />
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

              <div className="control-group">
                <label>
                  <input
                    type="checkbox"
                    checked={!selectedBlock.is_disabled}
                    onChange={event => updateZoomBlock(selectedBlock.id, { is_disabled: !event.target.checked })}
                    disabled={isExporting}
                  />{' '}Enable zoom
                </label>
                <small className="setting-hint">Keep this block’s settings while turning its effect on or off.</small>
              </div>

              <ZoomPresentationControls block={selectedBlock} disabled={isExporting} />

              <div className="control-group">
                <label className="checkbox-control">
                  <input type="checkbox" checked={(visualSettings.zoomLeadInMs ?? 0) > 0}
                    disabled={isExporting}
                    onChange={event => updateVisualSettings({ zoomLeadInMs: event.target.checked ? 350 : 0 })} />
                  Start zooms early
                </label>
                <small className="setting-hint">
                  Applies to every zoom in this project. Lead-in: {visualSettings.zoomLeadInMs ?? 0} ms.
                  {' '}Adjust the timing in Motion. Instant zooms keep their original start.
                </small>
              </div>

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
                      updateZoomBlock(selectedBlock.id, {
                        zoom_factor: newZoomFactor,
                        ...manualZoomCenterUpdate(selectedBlock, newZoomFactor, captureMode, !!selectedBlock.loupe),
                      });
                    }}
                    className="editor-slider"
                    disabled={isExporting}
                  />
                  <span className="value-display">{selectedBlock.zoom_factor.toFixed(1)}x</span>
                </div>
              </div>

              {selectedBlock.kind && (
                <div className="control-group">
                  <label>Block Type</label>
                  <span className={`confidence-badge ${selectedBlock.kind === 'typing' ? 'confidence-high' : 'confidence-medium'}`}
                    style={{ textTransform: 'capitalize' }}>
                    {selectedBlock.kind}
                  </span>
                </div>
              )}

              <div className="control-group">
                <label>
                  <input
                    type="checkbox"
                    checked={selectedBlock.instant_animation ?? false}
                    onChange={event => updateZoomBlock(selectedBlock.id, { instant_animation: event.target.checked })}
                    disabled={isExporting}
                  />{' '}Instant zoom
                </label>
                <small className="setting-hint">Snap to the zoom and back without an animated transition.</small>
              </div>

              {!selectedBlock.loupe && <>
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
                    : `Using global: ${visualSettings.screenMotionStyle ?? visualSettings.zoomSpeedPreset}`}
                </small>
              </div>

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
                    : `Using global: ${visualSettings.screenMotionStyle ?? visualSettings.zoomSpeedPreset}`}
                </small>
              </div>

              </>}

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
                    aspectRatio: pickerAspect,
                    background: 'var(--editor-bg-tertiary)',
                    borderRadius: '6px',
                    border: '1px solid var(--editor-border)',
                    cursor: isExporting ? 'not-allowed' : 'crosshair',
                    overflow: 'hidden',
                    marginTop: '8px',
                    opacity: isExporting ? 0.5 : 1
                  }}
                >
                  <div style={{
                    position: 'absolute',
                    inset: 0,
                    backgroundImage: 'linear-gradient(var(--editor-border) 1px, transparent 1px), linear-gradient(90deg, var(--editor-border) 1px, transparent 1px)',
                    backgroundSize: '25% 25%',
                    opacity: 0.3
                  }} />

                  <div
                    style={{
                      position: 'absolute',
                      left: `${clickPosition.x * 100}%`,
                      top: `${clickPosition.y * 100}%`,
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

                  <div
                    style={{
                      position: 'absolute',
                      left: `${pickerCenter.x * 100}%`,
                      top: `${pickerCenter.y * 100}%`,
                      width: selectedBlock.loupe ? '24px' : `${zoomWindowSize}%`,
                      height: selectedBlock.loupe ? '24px' : `${zoomWindowSize}%`,
                      border: '2px solid var(--accent-primary)',
                      borderRadius: selectedBlock.loupe ? '50%' : '4px',
                      transform: 'translate(-50%, -50%)',
                      background: 'rgba(189, 147, 249, 0.1)',
                      boxShadow: selectedBlock.loupe ? 'none' : '0 0 0 2000px rgba(0,0,0,0.4)',
                      pointerEvents: 'none'
                    }}
                  />

                  <div
                    style={{
                      position: 'absolute',
                      left: `${pickerCenter.x * 100}%`,
                      top: `${pickerCenter.y * 100}%`,
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
                <p>{zoomAnalysis.zoom_blocks.length ? 'No block selected' : 'No zoom blocks yet'}</p>
                <span>
                  {zoomAnalysis.zoom_blocks.length
                    ? `${zoomAnalysis.zoom_blocks.length} zoom blocks available. Click one in the timeline to edit.`
                    : 'Rebuild automatic zooms from recorded input, or add a block from the toolbar.'}
                </span>
              </div>
            </div>
          )}
        </>
      )}

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
