import { sourceTimeResolver } from '../../../utils/timeline/sourceTimeResolver';
import { readZoomStyle } from '../../../utils/motion/zoomStylePreference';
import { useShallow } from 'zustand/react/shallow';
import { timelineStructureState } from './structureState';
import { TimelineTime, CollapsedPlayhead } from './TimelineClock';
import MusicTrack from './MusicTrack';
import { projectSourceRange, newSourceZoomRange, indexZoomRangeLimits, zoomResizeAvailability } from '../../../utils/timeline/sourceRanges';
import { splitTimelineAtTime } from '../../../utils/timeline/splitTimeline';
import { timelineDuration } from '../../../utils/timeline/previewTimeline';
import React, { useRef, useState, useEffect, useLayoutEffect, useCallback, useMemo } from 'react';
import { Maximize2 } from 'lucide-react';
import { useEditorStore } from '../../../stores/editor';
import { useTimelineDrag, useZoomBlockDrag } from './hooks/useTimelineDrag';
import TimelineHeader from './TimelineHeader';
import VideoTrack from './VideoTrack';
import ZoomTrack from './ZoomTrack';
import WebcamTrack from './WebcamTrack';
import AudioTrack from './AudioTrack';
import Playhead from './Playhead';
import { calculateTimelineLayout, fitTimelineZoom, stepTimelineZoom } from './utils';
import './timeline.css';

interface ProfessionalTimelineProps {
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  isExporting?: boolean;
}

const ProfessionalTimeline: React.FC<ProfessionalTimelineProps> = ({
  isCollapsed = false,
  onToggleCollapse,
  isExporting = false
}) => {
  const timelineRef = useRef<HTMLDivElement>(null);
  const [timelineZoom, setTimelineZoom] = useState(1);
  const [timelineScrollLeft, setTimelineScrollLeft] = useState(0);
  const [timelineViewportWidth, setTimelineViewportWidth] = useState(0);

  const store = useEditorStore(useShallow(timelineStructureState));

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
    setCurrentTime,
    trimStart,
    trimEnd: storedTrimEnd,
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
    selection,
    selectClips,
    clearSelection,
    currentTool,
    setCurrentTool,
    snappingEnabled,
    setSnappingEnabled,
    hasMicrophone,
    hasSystemAudio,
    hasWebcam,
  } = store;

  const duration = timelineDuration(store);
  const trimEnd = Math.min(storedTrimEnd, duration);
  const trackHeaderWidth = 140;
  const timelineLayout = calculateTimelineLayout(
    duration,
    timelineViewportWidth,
    trackHeaderWidth,
    timelineZoom,
  );
  const availableTimelineWidth = timelineLayout.availableWidth;
  const pixelsPerMs = timelineLayout.pixelsPerMs;
  const timelineWidth = timelineLayout.mediaWidth;
  const timelineTracksWidth = timelineLayout.tracksWidth;
  const audioBasePath = videoFilePath?.replace(/\.[^/.]+$/, '') ?? null;

  useLayoutEffect(() => {
    const timeline = timelineRef.current;
    if (!timeline) return;

    const updateViewportWidth = () => {
      const width = timeline.clientWidth;
      setTimelineViewportWidth((current) => current === width ? current : width);
    };

    updateViewportWidth();
    const observer = new ResizeObserver(updateViewportWidth);
    observer.observe(timeline);
    return () => observer.disconnect();
  }, []);

  const getVideoElement = useCallback((): HTMLVideoElement | null => {
    return window.__TARANTINO_VIDEO_ELEMENT || null;
  }, []);

  const seekVideo = useCallback((timeMs: number) => {
    setCurrentTime(timeMs);
  }, [setCurrentTime]);

  const getTimeFromClientX = useCallback((clientX: number) => {
    if (!timelineRef.current) return useEditorStore.getState().currentTime;
    const rect = timelineRef.current.getBoundingClientRect();
    const x = clientX - rect.left + timelineRef.current.scrollLeft - trackHeaderWidth;
    return Math.max(0, Math.min(duration, x / pixelsPerMs));
  }, [duration, pixelsPerMs]);

  const splitAtTime = useCallback((time: number, zoomId: string | null = selectedBlockId) => {
    if (isExporting) return;
    const newClipIds = splitTimelineAtTime(useEditorStore.getState(), time, zoomId);
    if (newClipIds.length > 0) {
      store.selectClips(newClipIds);
      setCurrentTool('select');
    }
    seekVideo(time);
  }, [isExporting, seekVideo, selectedBlockId, setCurrentTool, store]);

  const setVideoPlaying = useCallback((playing: boolean) => {
    const playFunction = window.__TARANTINO_SET_PLAYING;
    if (playFunction) {
      playFunction(playing);
    }
  }, []);

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
  const zoomBlocks = useMemo(() => {
    if (!hasValidZoomData) return [];
    const blocks = zoomAnalysis!.zoom_blocks;
    const limits = indexZoomRangeLimits(blocks, store.duration);
    return blocks.flatMap(block =>
      projectSourceRange(store, block.start_time, block.end_time).map(range => ({
        ...range, id:block.id, zoomFactor:block.zoom_factor, isManual:block.is_manual,
        canResize: zoomResizeAvailability(range, limits.get(block.id)!,
          sourceTimeResolver(store, range.startTime)),
      })));
  }, [zoomAnalysis, store.clips, store.tracks, store.duration]);
  const mappedZoomKeyframes = useMemo(() => zoomKeyframes.flatMap(kf => projectSourceRange(store,kf.time,kf.time+kf.duration)
    .map(range => ({...kf,id:`${kf.id}:${range.occurrenceId}`,time:range.startTime,duration:range.endTime-range.startTime}))), [zoomKeyframes, store.clips, store.tracks]);
  const mappedPreviewZoom = useMemo(() => previewZoomAnalysis && {...previewZoomAnalysis, indicators:previewZoomAnalysis.indicators.flatMap(indicator =>
    projectSourceRange(store,indicator.preview_start,indicator.preview_end).map(range => ({...indicator,
      id:`${indicator.id}:${range.occurrenceId}`,preview_start:range.startTime,preview_end:range.endTime,
      click_time:range.startTime+(indicator.click_time-range.sourceStart)/range.rate,
    })))}, [previewZoomAnalysis, store.clips, store.tracks]);

  const { handleDragStart, handleResizeStart } = useZoomBlockDrag({
    pixelsPerMs,
    duration: store.duration,
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
        case 'x':
          if (!isExporting && !e.repeat && !e.metaKey && !e.ctrlKey && !e.altKey
            && !(e.target instanceof HTMLElement && e.target.isContentEditable)) {
            store.removePreviousClip();
            e.preventDefault();
          }
          break;
        case 'v':
          setCurrentTool('select');
          e.preventDefault();
          break;
        case 'c':
          if (!isExporting) {
            setCurrentTool('scissors');
          }
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
          setTimelineZoom(prev => stepTimelineZoom(prev,'in',duration,availableTimelineWidth));
          e.preventDefault();
          break;
        case '-':
          setTimelineZoom(prev => stepTimelineZoom(prev,'out',duration,availableTimelineWidth));
          e.preventDefault();
          break;
        case '0':
          setTimelineZoom(fitTimelineZoom(duration,availableTimelineWidth));
          e.preventDefault();
          break;
        case ' ': {
          const playback = useEditorStore.getState();
          const wantPlay = !playback.isPlaying;
          if (wantPlay && playback.currentTime >= duration) {
            seekVideo(0);
          }
          setVideoPlaying(wantPlay);
          e.preventDefault();
          break;
        }
        case 'backspace':
        case 'delete':
          if (!isExporting) {
            if (selection.clipIds.length > 0) {
              store.beginHistoryGroup();
              try { selection.clipIds.forEach((clipId) => store.deleteClip(clipId)); }
              finally { store.endHistoryGroup(); }
              clearSelection();
            } else if (selectedBlockId) {
              deleteZoomBlock(selectedBlockId);
            }
            e.preventDefault();
          }
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [availableTimelineWidth, snappingEnabled, isExporting, setCurrentTool, setSnappingEnabled, setVideoPlaying, getVideoElement, duration, seekVideo, splitAtTime, selection.clipIds, store, clearSelection, selectedBlockId, deleteZoomBlock]);

  const handleTimelineClick = (e: React.MouseEvent) => {
    if (!timelineRef.current || isDragging) return;

    const time = getTimeFromClientX(e.clientX);

    if (selectedBlockId) {
      setSelectedBlockId(null);
    }

    if (currentTool === 'scissors' && !isExporting) {
      splitAtTime(time, null);
    } else {
      clearSelection();
      setCurrentTime(time);
      seekVideo(time);
    }

    e.stopPropagation();
  };

  const handleClipClick = (clipId: string, event: React.MouseEvent) => {
    if (isExporting) return;
    event.stopPropagation();

    const time = getTimeFromClientX(event.clientX);
    if (currentTool === 'scissors') {
      splitAtTime(time, null);
      return;
    }

    if (selectedBlockId) {
      setSelectedBlockId(null);
    }
    selectClips([clipId], event.shiftKey || event.metaKey);
    setCurrentTime(time);
    seekVideo(time);
  };

  const handleBlockClick = (blockId: string, event: React.MouseEvent) => {
    if (isExporting) return;
    event.stopPropagation();
    if (currentTool === 'scissors') {
      splitAtTime(getTimeFromClientX(event.clientX), blockId);
      return;
    }
    clearSelection();
    setSelectedBlockId(blockId);
  };

  const handleBlockDoubleClick = (blockId: string, event: React.MouseEvent) => {
    if (isExporting) return;
    event.stopPropagation();
    deleteZoomBlock(blockId);
  };

  const handleAddZoomBlock = () => {
    if (isExporting) return;
    const range = newSourceZoomRange(store, useEditorStore.getState().currentTime);
    if (!range) return;
    if (range.splitBlockId) {
      store.splitZoomBlockAtTime(range.splitBlockId, range.start_time);
      const inserted = useEditorStore.getState().zoomAnalysis?.zoom_blocks.find(
        block => block.start_time === range.start_time && block.end_time === range.end_time);
      if (inserted) setSelectedBlockId(inserted.id);
      clearSelection();
      return;
    }
    const id = crypto.randomUUID();
    store.addZoomBlock({
      id,
      click_x: 0.5,
      click_y: 0.5,
      center_x: 0.5,
      center_y: 0.5,
      start_time: range.start_time,
      end_time: range.end_time,
      ...readZoomStyle(),
      is_manual: true
    });
    clearSelection();
    setSelectedBlockId(id);
  };

  const fitTimeline = () => setTimelineZoom(fitTimelineZoom(duration,availableTimelineWidth));

  const videoVisible = store.tracks.some(track =>
    (track.type === 'video' || track.type === 'screen') && track.visible);
  const toggleVideoVisibility = () => {
    if (!isExporting) store.setVideoTracksVisible(!videoVisible);
  };

  // Collapsed view
  if (isCollapsed) {
    return (
      <div className="timeline-collapsed">
        <CollapsedPlayhead duration={duration} />
        <div className="collapsed-controls">
          <TimelineTime className="time-display" />
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
        duration={duration}
        currentTool={currentTool}
        setCurrentTool={setCurrentTool}
        snappingEnabled={snappingEnabled}
        setSnappingEnabled={setSnappingEnabled}
        onZoomIn={() => setTimelineZoom(prev => stepTimelineZoom(prev,'in',duration,availableTimelineWidth))}
        onZoomOut={() => setTimelineZoom(prev => stepTimelineZoom(prev,'out',duration,availableTimelineWidth))}
        onFitTimeline={fitTimeline}
        onToggleCollapse={onToggleCollapse}
        onAddZoomBlock={handleAddZoomBlock}
        onSplitAtPlayhead={() => splitAtTime(useEditorStore.getState().currentTime)}
        onRemovePreviousClip={() => store.removePreviousClip()}
        isExporting={isExporting}
      />

      <div
        className={`timeline-content ${cursorClass}`}
        ref={timelineRef}
        onScroll={event => setTimelineScrollLeft(event.currentTarget.scrollLeft)}
        onClick={handleTimelineClick}
      >
        <div className="timeline-tracks" style={{ width: `${Math.max(timelineTracksWidth, 100)}px` }}>
          {(
            <VideoTrack
              timelineWidth={timelineWidth}
              viewportLeft={Math.max(0,timelineScrollLeft-trackHeaderWidth-100)}
              viewportRight={timelineScrollLeft+Math.max(0,timelineViewportWidth-trackHeaderWidth)+100}
              pixelsPerMs={pixelsPerMs}
              sourceDuration={store.duration}
              sourcePath={videoFilePath}
              tracks={store.tracks}
              thumbnails={thumbnails}
              thumbnailsLoading={thumbnailsLoading}
              trimStart={trimStart}
              trimEnd={trimEnd}
              duration={duration}
              clips={clips}
              selectedClipIds={selection.clipIds}
              isExporting={isExporting}
              visible={videoVisible}
              onToggleVisibility={toggleVideoVisibility}
              onTrimStartDrag={(e) => handleMouseDown(e, 'trim-start')}
              onTrimEndDrag={(e) => handleMouseDown(e, 'trim-end')}
              onClipClick={handleClipClick}
            />
          )}

          {(
            <ZoomTrack
              pixelsPerMs={pixelsPerMs}
              zoomBlocks={zoomBlocks}
              zoomKeyframes={mappedZoomKeyframes}
              previewZoomAnalysis={mappedPreviewZoom}
              zoomLoading={zoomLoading}
              previewZoomLoading={previewZoomLoading}
              zoomAnalysis={zoomAnalysis}
              selectedBlockId={selectedBlockId}
              isExporting={isExporting}
              effectsEnabled={store.visualSettings.zoomEffectsEnabled !== false}
              onToggleVisibility={() => store.updateVisualSettings({zoomEffectsEnabled: store.visualSettings.zoomEffectsEnabled === false})}
              onBlockClick={handleBlockClick}
              onBlockDoubleClick={handleBlockDoubleClick}
              onDragStart={(e, block) => {
                if (currentTool === 'scissors') return;
                clearSelection();
                handleDragStart(e, block);
                setSelectedBlockId(block.id);
              }}
              onResizeStart={handleResizeStart}
            />
          )}

          {hasWebcam && (
            <WebcamTrack
              pixelsPerMs={pixelsPerMs}
              webcamKeyframes={webcamKeyframes.flatMap(k=>projectSourceRange(store,k.time,k.time+0.001).map(r=>({...k,id:`${k.id}:${r.occurrenceId}`,time:r.startTime})))}
              isExporting={isExporting}
              enabled={store.visualSettings.webcamEnabled !== false}
              onToggleVisibility={() => store.updateVisualSettings({webcamEnabled: store.visualSettings.webcamEnabled === false})}
            />
          )}

          {hasMicrophone && (
            <AudioTrack
              viewportLeft={Math.max(0,timelineScrollLeft-trackHeaderWidth-100)}
              viewportRight={timelineScrollLeft+Math.max(0,timelineViewportWidth-trackHeaderWidth)+100}
              type="microphone"
              duration={duration}
              pixelsPerMs={pixelsPerMs}
              audioPath={audioBasePath ? `${audioBasePath}.mic.wav` : null}
              isExporting={isExporting}
              muted={store.audioSettings.micMuted ?? false}
              onToggleMute={() => store.updateAudioSettings({micMuted: !store.audioSettings.micMuted})}
            />
          )}

          {hasSystemAudio && (
            <AudioTrack
              viewportLeft={Math.max(0,timelineScrollLeft-trackHeaderWidth-100)}
              viewportRight={timelineScrollLeft+Math.max(0,timelineViewportWidth-trackHeaderWidth)+100}
              type="system"
              duration={duration}
              pixelsPerMs={pixelsPerMs}
              audioPath={audioBasePath ? `${audioBasePath}.system.wav` : null}
              isExporting={isExporting}
              muted={store.audioSettings.systemMuted ?? false}
              onToggleMute={() => store.updateAudioSettings({systemMuted: !store.audioSettings.systemMuted})}
            />
          )}

          {store.audioSettings.backgroundAudio && <MusicTrack music={store.audioSettings.backgroundAudio}
            duration={duration} pixelsPerMs={pixelsPerMs} disabled={isExporting}
            left={Math.max(0,timelineScrollLeft-trackHeaderWidth-100)}
            right={timelineScrollLeft+Math.max(0,timelineViewportWidth-trackHeaderWidth)+100}/>}
          <Playhead
                pixelsPerMs={pixelsPerMs}
            trackHeaderWidth={trackHeaderWidth}
            onMouseDown={(e) => handleMouseDown(e, 'playhead')}
          />
        </div>
      </div>
    </div>
  );
};

export default React.memo(ProfessionalTimeline);
