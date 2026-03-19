import React from 'react';
import { Search, Eye } from 'lucide-react';
import type { ZoomKeyframe, ZoomAnalysis, PreviewZoomAnalysis } from '../../../stores/editor/types';

interface ZoomBlock {
  id: string;
  startTime: number;
  endTime: number;
  zoomFactor: number;
  isManual: boolean;
}

interface ZoomTrackProps {
  pixelsPerMs: number;
  zoomBlocks: ZoomBlock[];
  zoomKeyframes: ZoomKeyframe[];
  previewZoomAnalysis: PreviewZoomAnalysis | null;
  zoomLoading: boolean;
  previewZoomLoading: boolean;
  zoomAnalysis: ZoomAnalysis | null;
  selectedBlockId: string | null;
  isExporting: boolean;
  onToggleVisibility: () => void;
  onBlockClick: (blockId: string, event: React.MouseEvent) => void;
  onBlockDoubleClick: (blockId: string, event: React.MouseEvent) => void;
  onDragStart: (event: React.MouseEvent, block: ZoomBlock) => void;
  onResizeStart: (event: React.MouseEvent, block: ZoomBlock, type: 'start' | 'end') => void;
}

const ZoomTrack: React.FC<ZoomTrackProps> = ({
  pixelsPerMs,
  zoomBlocks,
  zoomKeyframes,
  previewZoomAnalysis,
  zoomLoading,
  previewZoomLoading,
  zoomAnalysis,
  selectedBlockId,
  isExporting,
  onToggleVisibility,
  onBlockClick,
  onBlockDoubleClick,
  onDragStart,
  onResizeStart,
}) => {
  const totalCount = zoomBlocks.length + (zoomKeyframes?.length || 0) + (previewZoomAnalysis?.indicators?.length || 0);
  const isEmpty = zoomBlocks.length === 0 && (!zoomKeyframes || zoomKeyframes.length === 0) && (!previewZoomAnalysis?.indicators || previewZoomAnalysis.indicators.length === 0);

  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label" title="Zoom effects from mouse clicks">
          <Search size={12} />
          <span>Zoom</span>
          <span className="track-count">{totalCount}</span>
        </div>
        <button
          className={`track-toggle ${isExporting ? 'disabled' : ''}`}
          onClick={onToggleVisibility}
          disabled={isExporting}
        >
          <Eye size={12} />
        </button>
      </div>
      <div className="track-content">
        {(zoomLoading || previewZoomLoading) ? (
          <div className="auto-zoom-loading">
            <div className="editor-spinner" />
            <span>{previewZoomLoading ? 'Analyzing clicks...' : 'Loading zoom data...'}</span>
          </div>
        ) : isEmpty ? (
          <div className="auto-zoom-empty">
            {zoomAnalysis && zoomAnalysis.total_clicks > 0 ? (
              <>
                <span>Found {zoomAnalysis.total_clicks} clicks but no valid zoom blocks generated</span>
                <small style={{ marginTop: '4px', color: 'var(--editor-text-secondary)' }}>
                  Clicks may be too close together or outside valid time range
                </small>
              </>
            ) : (
              <>
                <span>No mouse click data found for smart zoom generation</span>
                <small style={{ marginTop: '4px', color: 'var(--editor-text-secondary)' }}>
                  Record with mouse clicks to automatically generate zoom effects
                </small>
              </>
            )}
          </div>
        ) : (
          <>
            {/* Preview zoom indicators */}
            {previewZoomAnalysis?.indicators?.map(indicator => {
              const hasProcessedBlockAtTime = zoomBlocks.some(block =>
                block.startTime <= indicator.click_time && block.endTime >= indicator.click_time
              );

              if (hasProcessedBlockAtTime) return null;

              return (
                <div
                  key={`preview_${indicator.id}`}
                  className="preview-zoom-indicator"
                  style={{
                    position: 'absolute',
                    left: `${indicator.preview_start * pixelsPerMs}px`,
                    width: `${(indicator.preview_end - indicator.preview_start) * pixelsPerMs}px`,
                    height: '20px',
                    top: '6px',
                    cursor: 'default',
                    userSelect: 'none',
                    zIndex: 1
                  }}
                  title={`Preview zoom at ${Math.round(indicator.click_time)}ms\nConfidence: ${Math.round(indicator.confidence * 100)}%`}
                >
                  <div
                    className="preview-zoom-body"
                    style={{
                      width: '100%',
                      height: '100%',
                      backgroundColor: '#6272a4',
                      borderRadius: '3px',
                      border: '1px dashed #8be9fd',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '9px',
                      fontWeight: '400',
                      color: '#f8f8f2',
                      opacity: 0.6 + (indicator.confidence * 0.4),
                      position: 'relative',
                      transition: 'all 0.2s ease'
                    }}
                  >
                    <span>~{(2.0).toFixed(1)}x</span>
                    <div
                      style={{
                        position: 'absolute',
                        left: `${((indicator.click_time - indicator.preview_start) / (indicator.preview_end - indicator.preview_start)) * 100}%`,
                        top: '50%',
                        transform: 'translate(-50%, -50%)',
                        width: '2px',
                        height: '12px',
                        backgroundColor: '#8be9fd',
                        borderRadius: '1px',
                        opacity: 0.8
                      }}
                    />
                  </div>
                </div>
              );
            })}

            {/* Auto-generated zoom blocks */}
            {zoomBlocks.map(block => {
              const isSelected = selectedBlockId === block.id;
              return (
                <div
                  key={block.id}
                  className={`zoom-block-container ${isSelected ? 'selected' : ''} ${isExporting ? 'disabled' : ''}`}
                  style={{
                    position: 'absolute',
                    left: `${block.startTime * pixelsPerMs}px`,
                    width: `${(block.endTime - block.startTime) * pixelsPerMs}px`,
                    height: '24px',
                    top: '4px',
                    cursor: isExporting ? 'not-allowed' : 'move',
                    userSelect: 'none',
                    zIndex: isSelected ? 3 : 2,
                    opacity: isExporting ? 0.6 : 1
                  }}
                  title={isExporting ? "Editing disabled during export" : `${block.zoomFactor.toFixed(1)}x zoom ${block.isManual ? '(Manual)' : '(Auto)'}`}
                  onClick={(e) => onBlockClick(block.id, e)}
                  onDoubleClick={(e) => onBlockDoubleClick(block.id, e)}
                  onMouseDown={(e) => onDragStart(e, block)}
                >
                  <div
                    className="zoom-block-body"
                    style={{
                      width: '100%',
                      height: '100%',
                      backgroundColor: block.isManual ? '#ff79c6' : '#bd93f9',
                      borderRadius: '4px',
                      border: isSelected
                        ? '2px solid #8be9fd'
                        : block.isManual
                          ? '2px solid #f8f8f2'
                          : '1px solid #9580ff',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '11px',
                      fontWeight: '500',
                      color: '#282a36',
                      position: 'relative',
                      boxShadow: isSelected
                        ? '0 0 12px rgba(139, 233, 253, 0.5)'
                        : '0 1px 3px rgba(0,0,0,0.3)',
                      transition: 'all 0.1s ease'
                    }}
                  >
                    <span>{block.zoomFactor.toFixed(1)}x</span>

                    {/* Left resize handle */}
                    <div
                      className="resize-handle resize-handle-left"
                      style={{
                        position: 'absolute',
                        left: '-2px',
                        top: '50%',
                        transform: 'translateY(-50%)',
                        width: '4px',
                        height: '16px',
                        backgroundColor: '#6272a4',
                        borderRadius: '2px',
                        cursor: 'ew-resize',
                        opacity: 0,
                        transition: 'opacity 0.1s ease'
                      }}
                      onMouseDown={(e) => onResizeStart(e, block, 'start')}
                    />

                    {/* Right resize handle */}
                    <div
                      className="resize-handle resize-handle-right"
                      style={{
                        position: 'absolute',
                        right: '-2px',
                        top: '50%',
                        transform: 'translateY(-50%)',
                        width: '4px',
                        height: '16px',
                        backgroundColor: '#6272a4',
                        borderRadius: '2px',
                        cursor: 'ew-resize',
                        opacity: 0,
                        transition: 'opacity 0.1s ease'
                      }}
                      onMouseDown={(e) => onResizeStart(e, block, 'end')}
                    />
                  </div>
                </div>
              );
            })}

            {/* Manual zoom keyframes */}
            {zoomKeyframes?.map(kf => (
              <div
                key={kf.id}
                className="zoom-keyframe"
                style={{
                  left: `${kf.time * pixelsPerMs}px`,
                  width: `${kf.duration * pixelsPerMs}px`,
                  top: '28px',
                  height: '12px'
                }}
                title={`${kf.scale}x manual zoom`}
              >
                <div className="keyframe-handle keyframe-handle--start" />
                <div className="keyframe-content">{kf.scale}x</div>
                <div className="keyframe-handle keyframe-handle--end" />
              </div>
            ))}

            {/* Zoom Legend */}
            <ZoomLegend
              zoomBlocks={zoomBlocks}
              zoomKeyframes={zoomKeyframes}
              previewZoomAnalysis={previewZoomAnalysis}
            />
          </>
        )}
      </div>
    </div>
  );
};

interface ZoomLegendProps {
  zoomBlocks: ZoomBlock[];
  zoomKeyframes: ZoomKeyframe[] | undefined;
  previewZoomAnalysis: PreviewZoomAnalysis | null;
}

const ZoomLegend: React.FC<ZoomLegendProps> = ({ zoomBlocks, zoomKeyframes, previewZoomAnalysis }) => {
  const hasContent = zoomBlocks.length > 0 || (zoomKeyframes && zoomKeyframes.length > 0) || (previewZoomAnalysis?.indicators && previewZoomAnalysis.indicators.length > 0);

  if (!hasContent) return null;

  return (
    <div className="zoom-legend" style={{
      position: 'absolute',
      right: '8px',
      top: '2px',
      display: 'flex',
      gap: '8px',
      fontSize: '10px',
      color: 'var(--editor-text-secondary)'
    }}>
      {zoomBlocks.some(b => !b.isManual) && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <div style={{ width: '8px', height: '8px', backgroundColor: '#bd93f9', borderRadius: '2px' }} />
          <span>Auto</span>
        </div>
      )}
      {zoomBlocks.some(b => b.isManual) && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <div style={{ width: '8px', height: '8px', backgroundColor: '#ff79c6', borderRadius: '2px' }} />
          <span>Manual</span>
        </div>
      )}
      {previewZoomAnalysis?.indicators && previewZoomAnalysis.indicators.length > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <div style={{ width: '8px', height: '8px', backgroundColor: '#6272a4', border: '1px dashed #8be9fd', borderRadius: '2px', opacity: 0.7 }} />
          <span>Preview</span>
        </div>
      )}
      {zoomKeyframes && zoomKeyframes.length > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          <div style={{ width: '8px', height: '4px', backgroundColor: 'var(--dracula-pink)', borderRadius: '2px' }} />
          <span>Keyframe</span>
        </div>
      )}
    </div>
  );
};

export default ZoomTrack;
