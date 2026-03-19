import React, { useRef, useState, useEffect, useCallback } from 'react';
import { Maximize2 } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import { useTimelineDrag, useZoomBlockDrag } from './hooks/useTimelineDrag';
import TimelineHeader, { formatTime } from './TimelineHeader';
import VideoTrack from './VideoTrack';
import ZoomTrack from './ZoomTrack';
import WebcamTrack from './WebcamTrack';
import AudioTrack from './AudioTrack';
import Playhead from './Playhead';
import './timeline.css';

interface ProfessionalTimelineProps {
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  isExporting?: boolean;
}

interface VisibleTracks {
  video: boolean;
  smartZoom: boolean;
  webcam: boolean;
  microphone: boolean;
  system: boolean;
}

const ProfessionalTimeline: React.FC<ProfessionalTimelineProps> = ({
  isCollapsed = false,
  onToggleCollapse,
  isExporting = false
}) => {
  const timelineRef = useRef<HTMLDivElement>(null);
  const [timelineZoom, setTimelineZoom] = useState(1);
  const [visibleTracks, setVisibleTracks] = useState<VisibleTracks>({
    video: true,
    smartZoom: true,
    webcam: true,
    microphone: true,
    system: true
  });

  const store = useEditorStore();

  // Early return if store is not initialized
  if (!store || !store.videoFilePath) {
    return (
      <div className="professional-timeline">
        <div style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100px',
          color: 'var(--editor-text-secondary)',
          background: 'var(--timeline-bg)'
        }}>
          Loading timeline...
        </div>
      </div>
    );
  }

  const {
    duration,
    currentTime,
    setCurrentTime,
    trimStart,
    trimEnd,
    setTrimStart,
    setTrimEnd,
    zoomKeyframes,
    webcamKeyframes,
    thumbnails,
    thumbnailsLoading,
    zoomAnalysis,
    zoomLoading,
    previewZoomAnalysis,
    previewZoomLoading,
    updateZoomBlock,
    deleteZoomBlock,
    videoFilePath,
    selectedBlockId,
    setSelectedBlockId,
    clips,
    currentTool,
    setCurrentTool,
    snappingEnabled,
    setSnappingEnabled,
    hasMicrophone,
    hasSystemAudio,
    hasWebcam,
  } = store;

  const pixelsPerMs = 0.1 * timelineZoom;
  const timelineWidth = duration * pixelsPerMs;
  const trackHeaderWidth = 140;

  // Video control helpers
  const getVideoElement = useCallback((): HTMLVideoElement | null => {
    return (window as any).__TARANTINO_VIDEO_ELEMENT || null;
  }, []);

  const seekVideo = useCallback((timeMs: number) => {
    const seekFunction = (window as any).__TARANTINO_SEEK_VIDEO;
    if (seekFunction) {
      seekFunction(timeMs);
    } else {
      const video = getVideoElement();
      if (video) {
        video.currentTime = timeMs / 1000;
      }
    }
  }, [getVideoElement]);

  const setVideoPlaying = useCallback((playing: boolean) => {
    const playFunction = (window as any).__TARANTINO_SET_PLAYING;
    if (playFunction) {
      playFunction(playing);
    }
  }, []);

  // Drag hooks
  const { isDragging, dragType, handleMouseDown } = useTimelineDrag({
    timelineRef,
    pixelsPerMs,
    duration,
    trackHeaderWidth,
    trimStart,
    trimEnd,
    setCurrentTime,
    setTrimStart,
    setTrimEnd,
    seekVideo,
    getVideoElement,
    isExporting,
  });

  // Get zoom blocks
  const hasValidZoomData = zoomAnalysis && zoomAnalysis.zoom_blocks && zoomAnalysis.zoom_blocks.length > 0;
  const zoomBlocks = hasValidZoomData ? zoomAnalysis!.zoom_blocks.map(block => ({
    id: block.id,
    startTime: block.start_time,
    endTime: block.end_time,
    zoomFactor: block.zoom_factor,
    isManual: block.is_manual
  })) : [];

  const { handleDragStart, handleResizeStart } = useZoomBlockDrag({
    pixelsPerMs,
    duration,
    updateZoomBlock,
    isExporting,
  });

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      switch (e.key.toLowerCase()) {
        case 'v':
          setCurrentTool('select');
          e.preventDefault();
          break;
        case 'c':
          if (!isExporting) setCurrentTool('scissors');
          e.preventDefault();
          break;
        case 't':
          if (!isExporting) setCurrentTool('trim');
          e.preventDefault();
          break;
        case 's':
          if (!isExporting) setCurrentTool('slip');
          e.preventDefault();
          break;
        case 'h':
          if (!isExporting) setCurrentTool('pan');
          e.preventDefault();
          break;
        case 'n':
          if (!isExporting) setSnappingEnabled(!snappingEnabled);
          e.preventDefault();
          break;
        case '+':
        case '=':
          setTimelineZoom(prev => Math.min(prev * 1.5, 10));
          e.preventDefault();
          break;
        case '-':
          setTimelineZoom(prev => Math.max(prev / 1.5, 0.1));
          e.preventDefault();
          break;
        case '0':
          setTimelineZoom(1);
          e.preventDefault();
          break;
        case ' ':
          setVideoPlaying(getVideoElement()?.paused ?? true);
          e.preventDefault();
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [snappingEnabled, isExporting, setCurrentTool, setSnappingEnabled, setVideoPlaying, getVideoElement]);

  const handleTimelineClick = (e: React.MouseEvent) => {
    if (!timelineRef.current || isDragging) return;

    const rect = timelineRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left - trackHeaderWidth;
    const time = Math.max(0, Math.min(duration, x / pixelsPerMs));

    if (selectedBlockId) {
      setSelectedBlockId(null);
    }

    if (currentTool === 'scissors' && !isExporting) {
      const clipsAtTime = store.getClipsAtTime(time);
      if (clipsAtTime.length > 0) {
        store.cutClipsAtTime(time);
      }
      setCurrentTool('select');
    } else {
      setCurrentTime(time);
      seekVideo(time);
    }

    e.stopPropagation();
  };

  const handleBlockClick = (blockId: string, event: React.MouseEvent) => {
    if (isExporting) return;
    event.stopPropagation();
    setSelectedBlockId(blockId);
  };

  const handleBlockDoubleClick = (blockId: string, event: React.MouseEvent) => {
    if (isExporting) return;
    event.stopPropagation();
    deleteZoomBlock(blockId);
  };

  const handleAddZoomBlock = () => {
    if (isExporting) return;
    const id = crypto.randomUUID();
    store.addZoomBlock({
      id,
      click_x: 0.5,
      click_y: 0.5,
      center_x: 0.5,
      center_y: 0.5,
      start_time: currentTime,
      end_time: currentTime + 2000,
      zoom_factor: 2.0,
      is_manual: true
    });
  };

  const toggleTrackVisibility = (track: keyof VisibleTracks) => {
    if (isExporting) return;
    setVisibleTracks(prev => ({ ...prev, [track]: !prev[track] }));
  };

  // Collapsed view
  if (isCollapsed) {
    return (
      <div className="timeline-collapsed">
        <div className="collapsed-playhead" style={{ left: `${(currentTime / duration) * 100}%` }} />
        <div className="collapsed-controls">
          <span className="time-display">{formatTime(currentTime)}</span>
          <button
            className="editor-btn editor-btn--ghost editor-btn--small"
            onClick={onToggleCollapse}
            title="Expand Timeline"
          >
            <Maximize2 size={14} />
          </button>
        </div>
      </div>
    );
  }

  const cursorClass = isExporting ? 'timeline-content--disabled' :
    currentTool === 'scissors' ? 'timeline-content--scissors' :
    currentTool === 'trim' ? 'timeline-content--trim' :
    currentTool === 'pan' ? 'timeline-content--pan' :
    'timeline-content--select';

  return (
    <div className="professional-timeline">
      <TimelineHeader
        currentTime={currentTime}
        duration={duration}
        currentTool={currentTool}
        setCurrentTool={setCurrentTool}
        snappingEnabled={snappingEnabled}
        setSnappingEnabled={setSnappingEnabled}
        onZoomIn={() => setTimelineZoom(prev => Math.min(prev * 1.5, 10))}
        onZoomOut={() => setTimelineZoom(prev => Math.max(prev / 1.5, 0.1))}
        onFitTimeline={() => setTimelineZoom(1)}
        onToggleCollapse={onToggleCollapse}
        onAddZoomBlock={handleAddZoomBlock}
        isExporting={isExporting}
      />

      <div
        className={`timeline-content ${cursorClass}`}
        ref={timelineRef}
        onClick={handleTimelineClick}
      >
        <div className="timeline-tracks" style={{ width: `${Math.max(timelineWidth, 100)}px` }}>
          {visibleTracks.video && (
            <VideoTrack
              timelineWidth={timelineWidth}
              pixelsPerMs={pixelsPerMs}
              thumbnails={thumbnails}
              thumbnailsLoading={thumbnailsLoading}
              trimStart={trimStart}
              trimEnd={trimEnd}
              duration={duration}
              clips={clips}
              isExporting={isExporting}
              onToggleVisibility={() => toggleTrackVisibility('video')}
              onTrimStartDrag={(e) => handleMouseDown(e, 'trim-start')}
              onTrimEndDrag={(e) => handleMouseDown(e, 'trim-end')}
            />
          )}

          {visibleTracks.smartZoom && (
            <ZoomTrack
              pixelsPerMs={pixelsPerMs}
              zoomBlocks={zoomBlocks}
              zoomKeyframes={zoomKeyframes}
              previewZoomAnalysis={previewZoomAnalysis}
              zoomLoading={zoomLoading}
              previewZoomLoading={previewZoomLoading}
              zoomAnalysis={zoomAnalysis}
              selectedBlockId={selectedBlockId}
              isExporting={isExporting}
              onToggleVisibility={() => toggleTrackVisibility('smartZoom')}
              onBlockClick={handleBlockClick}
              onBlockDoubleClick={handleBlockDoubleClick}
              onDragStart={(e, block) => {
                handleDragStart(e, block);
                setSelectedBlockId(block.id);
              }}
              onResizeStart={handleResizeStart}
            />
          )}

          {hasWebcam && visibleTracks.webcam && (
            <WebcamTrack
              pixelsPerMs={pixelsPerMs}
              webcamKeyframes={webcamKeyframes}
              isExporting={isExporting}
              onToggleVisibility={() => toggleTrackVisibility('webcam')}
            />
          )}

          {hasMicrophone && visibleTracks.microphone && (
            <AudioTrack
              type="microphone"
              duration={duration}
              pixelsPerMs={pixelsPerMs}
              audioPath={videoFilePath ? `${videoFilePath}.mic.wav` : null}
              isExporting={isExporting}
              onToggleVisibility={() => toggleTrackVisibility('microphone')}
            />
          )}

          {hasSystemAudio && visibleTracks.system && (
            <AudioTrack
              type="system"
              duration={duration}
              pixelsPerMs={pixelsPerMs}
              audioPath={videoFilePath ? `${videoFilePath}.system.wav` : null}
              isExporting={isExporting}
              onToggleVisibility={() => toggleTrackVisibility('system')}
            />
          )}

          <Playhead
            currentTime={currentTime}
            pixelsPerMs={pixelsPerMs}
            trackHeaderWidth={trackHeaderWidth}
            onMouseDown={(e) => handleMouseDown(e, 'playhead')}
          />
        </div>
      </div>
    </div>
  );
};

export default ProfessionalTimeline;
