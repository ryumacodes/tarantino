import TypingMarkers from './TypingMarkers';
import { useSourceThumbnails } from './hooks/useSourceThumbnails';
import { useClipTrim } from './hooks/useClipTrim';
import React, { useMemo } from 'react';
import { thumbnailTiles } from '../../../utils/timeline/thumbnailTiles';
import { Film, Eye, EyeOff } from 'lucide-react';

import type { TimelineClip, TimelineTrack } from '../../../stores/editor/types';

interface VideoTrackProps {
  timelineWidth: number;
  viewportLeft: number;
  viewportRight: number;
  pixelsPerMs: number;
  thumbnails: string[];
  thumbnailsLoading: boolean;
  trimStart: number;
  trimEnd: number;
  duration: number;
  sourceDuration: number;
  sourcePath: string | null;
  tracks: TimelineTrack[];
  clips: TimelineClip[];
  selectedClipIds: string[];
  isExporting: boolean;
  visible: boolean;
  onToggleVisibility: () => void;
  onTrimStartDrag: (e: React.MouseEvent) => void;
  onTrimEndDrag: (e: React.MouseEvent) => void;
  onClipClick: (clipId: string, e: React.MouseEvent) => void;
}

const VideoTrack: React.FC<VideoTrackProps> = ({
  timelineWidth,
  viewportLeft,
  viewportRight,
  pixelsPerMs,
  thumbnails,
  thumbnailsLoading,
  trimStart,
  trimEnd,
  duration,
  sourceDuration,
  sourcePath,
  tracks,
  clips,
  selectedClipIds,
  isExporting,
  visible,
  onToggleVisibility,
  onTrimStartDrag,
  onTrimEndDrag,
  onClipClick,
}) => {
  const beginClipTrim = useClipTrim(pixelsPerMs, isExporting);
  const videoClips = clips?.filter(c => c.type === 'video' || c.type === 'screen') || [];

  const tiles = useMemo(() => thumbnailTiles({clips,tracks,duration:sourceDuration},sourcePath,thumbnails.length,pixelsPerMs,100,{left:viewportLeft,right:viewportRight}),
    [clips,tracks,sourceDuration,sourcePath,thumbnails.length,pixelsPerMs,viewportLeft,viewportRight]);

  const sourceThumbnail = useSourceThumbnails(sourcePath, tiles);

  return (
    <div className="timeline-track">
      <div className="track-header">
        <div className="track-label">
          <Film size={12} />
          <span>Video</span>
        </div>
        <button
          className={`track-toggle ${isExporting ? 'disabled' : ''}`}
          aria-label="Enable video tracks"
          aria-pressed={visible}
          title={visible ? 'Hide video tracks' : 'Show video tracks'}
          onClick={event => { event.stopPropagation(); onToggleVisibility(); }}
          disabled={isExporting}
        >
          {visible ? <Eye size={12} /> : <EyeOff size={12} />}
        </button>
      </div>
      <div className="track-content" style={{ opacity: visible ? 1 : 0.45 }}>
        {/* Video thumbnails */}
        <div className="video-thumbnails timeline-track--golden" style={{ width: `${timelineWidth}px` }}>
          {thumbnailsLoading ? (
            <div className="thumbnails-loading">
              <div className="editor-spinner" />
            </div>
          ) : (
            tiles.filter(tile => !tile.needsSourceSample || sourceThumbnail(tile)).map(tile => (
              <img key={tile.key} src={tile.needsSourceSample ? sourceThumbnail(tile) : thumbnails[tile.index]}
                alt={`Source frame near ${(tile.sourceTime/1000).toFixed(1)} seconds`}
                className="video-thumbnail" draggable={false}
                style={{left:`${tile.left}px`,width:`${tile.width}px`,height:'60px',objectFit:'cover'}} />
            ))
          )}
        </div>

        <TypingMarkers pixelsPerMs={pixelsPerMs} viewportLeft={viewportLeft} viewportRight={viewportRight}
          trimStart={trimStart} trimEnd={trimEnd} isExporting={isExporting} />

        {/* Trim handles */}
        <div
          className={`trim-handle trim-handle--start ${isExporting ? 'disabled' : ''}`}
          style={{
            left: `${trimStart * pixelsPerMs}px`,
            cursor: isExporting ? 'not-allowed' : 'ew-resize',
            opacity: isExporting ? 0.4 : 0.8
          }}
          onMouseDown={onTrimStartDrag}
        />
        <div
          className={`trim-handle trim-handle--end ${isExporting ? 'disabled' : ''}`}
          style={{
            left: `${trimEnd * pixelsPerMs}px`,
            cursor: isExporting ? 'not-allowed' : 'ew-resize',
            opacity: isExporting ? 0.4 : 0.8
          }}
          onMouseDown={onTrimEndDrag}
        />

        {/* Trimmed overlays */}
        <div
          className="trim-overlay trim-overlay--start"
          style={{ width: `${trimStart * pixelsPerMs}px` }}
        />
        <div
          className="trim-overlay trim-overlay--end"
          style={{
            left: `${trimEnd * pixelsPerMs}px`,
            width: `${(duration - trimEnd) * pixelsPerMs}px`
          }}
        />

        {/* Clip boundaries */}
        {videoClips.map((clip, index) => {
          const isSelected = selectedClipIds.includes(clip.id);

          return (
            <div
              key={`clip-region-${clip.id}`}
              className={`clip-region ${isSelected ? 'selected' : ''} ${isExporting ? 'disabled' : ''}`}
              style={{
                left: `${clip.startTime * pixelsPerMs}px`,
                width: `${clip.duration * pixelsPerMs}px`,
              }}
              onClick={(e) => onClipClick(clip.id, e)}
              title={`${index === 0 ? 'Clip' : `Clip ${index + 1}`} • ${(clip.duration / 1000).toFixed(2)}s`}
            >
              {isSelected && (['start','end'] as const).map(side => (
                <button key={side} className={`clip-trim-handle clip-trim-handle--${side}`}
                  aria-label={`Trim clip ${index+1} ${side}`} title={`Trim clip ${side}`}
                  disabled={isExporting || clip.locked}
                  onClick={event => event.stopPropagation()}
                  onMouseDown={event => beginClipTrim(event,clip.id,side)} />
              ))}
            </div>
          );
        })}

        {videoClips.length > 1 && videoClips.map((clip, index) => {
          if (index === 0) return null;
          return (
            <div
              key={`clip-boundary-${clip.id}`}
              className="clip-boundary"
              style={{ left: `${clip.startTime * pixelsPerMs}px` }}
              title={`Cut at ${(clip.startTime / 1000).toFixed(2)}s`}
            />
          );
        })}
      </div>
    </div>
  );
};

export default VideoTrack;
