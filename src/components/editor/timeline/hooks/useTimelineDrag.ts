import { useState, useEffect, useCallback, RefObject } from 'react';

export type DragType = 'playhead' | 'trim-start' | 'trim-end' | null;

interface UseTimelineDragOptions {
  timelineRef: RefObject<HTMLDivElement | null>;
  pixelsPerMs: number;
  duration: number;
  trackHeaderWidth: number;
  trimStart: number;
  trimEnd: number;
  setCurrentTime: (time: number) => void;
  setTrimStart: (time: number) => void;
  setTrimEnd: (time: number) => void;
  seekVideo: (timeMs: number) => void;
  getVideoElement: () => HTMLVideoElement | null;
  isExporting: boolean;
}

interface UseTimelineDragResult {
  isDragging: boolean;
  dragType: DragType;
  handleMouseDown: (e: React.MouseEvent, type: DragType) => void;
}

export function useTimelineDrag({
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
}: UseTimelineDragOptions): UseTimelineDragResult {
  const [isDragging, setIsDragging] = useState(false);
  const [dragType, setDragType] = useState<DragType>(null);

  const handleMouseDown = useCallback((e: React.MouseEvent, type: DragType) => {
    // Disable trim editing during export (but allow playhead seeking)
    if (isExporting && (type === 'trim-start' || type === 'trim-end')) {
      return;
    }
    e.preventDefault();
    setIsDragging(true);
    setDragType(type);
  }, [isExporting]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (!timelineRef.current || !dragType) return;

      const rect = timelineRef.current.getBoundingClientRect();
      const x = e.clientX - rect.left - trackHeaderWidth;
      const time = Math.max(0, Math.min(duration, x / pixelsPerMs));

      switch (dragType) {
        case 'playhead':
          setCurrentTime(time);
          seekVideo(time);

          // Pause video during scrubbing for better performance
          const video = getVideoElement();
          if (video && !video.paused) {
            video.pause();
            (window as any).__TARANTINO_WAS_PLAYING = true;
          }
          break;
        case 'trim-start':
          setTrimStart(Math.min(time, trimEnd - 100));
          break;
        case 'trim-end':
          setTrimEnd(Math.max(time, trimStart + 100));
          break;
      }
    };

    const handleMouseUp = () => {
      // Resume playback if it was playing before scrubbing
      if (dragType === 'playhead' && (window as any).__TARANTINO_WAS_PLAYING) {
        const video = getVideoElement();
        if (video && video.paused) {
          video.play().catch(console.error);
        }
        (window as any).__TARANTINO_WAS_PLAYING = false;
      }

      setIsDragging(false);
      setDragType(null);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, dragType, timelineRef, pixelsPerMs, duration, trackHeaderWidth, trimStart, trimEnd, setCurrentTime, setTrimStart, setTrimEnd, seekVideo, getVideoElement]);

  return { isDragging, dragType, handleMouseDown };
}

// Zoom block drag hook
interface ZoomBlock {
  id: string;
  startTime: number;
  endTime: number;
  zoomFactor: number;
  isManual: boolean;
}

interface UseZoomBlockDragOptions {
  pixelsPerMs: number;
  duration: number;
  updateZoomBlock: (id: string, updates: { start_time?: number; end_time?: number }) => void;
  isExporting: boolean;
}

interface DraggingBlockState {
  blockId: string;
  startX: number;
  originalStart: number;
  originalEnd: number;
}

interface ResizingBlockState {
  blockId: string;
  type: 'start' | 'end';
  startX: number;
  originalStart: number;
  originalEnd: number;
}

export function useZoomBlockDrag({
  pixelsPerMs,
  duration,
  updateZoomBlock,
  isExporting,
}: UseZoomBlockDragOptions) {
  const [draggingBlock, setDraggingBlock] = useState<DraggingBlockState | null>(null);
  const [resizingBlock, setResizingBlock] = useState<ResizingBlockState | null>(null);

  const handleDragStart = useCallback((event: React.MouseEvent, block: ZoomBlock) => {
    if (isExporting) return;

    const target = event.target as HTMLElement;
    if (target.classList.contains('resize-handle') ||
        target.classList.contains('resize-handle-left') ||
        target.classList.contains('resize-handle-right')) {
      return;
    }

    event.stopPropagation();
    event.preventDefault();

    setDraggingBlock({
      blockId: block.id,
      startX: event.clientX,
      originalStart: block.startTime,
      originalEnd: block.endTime,
    });
  }, [isExporting]);

  const handleResizeStart = useCallback((event: React.MouseEvent, block: ZoomBlock, type: 'start' | 'end') => {
    if (isExporting) return;
    event.stopPropagation();
    event.preventDefault();

    setResizingBlock({
      blockId: block.id,
      type,
      startX: event.clientX,
      originalStart: block.startTime,
      originalEnd: block.endTime,
    });
  }, [isExporting]);

  // Handle resize
  useEffect(() => {
    if (!resizingBlock) return;

    const handleMouseMove = (event: MouseEvent) => {
      const deltaX = event.clientX - resizingBlock.startX;
      const deltaTime = deltaX / pixelsPerMs;

      if (resizingBlock.type === 'start') {
        const newStart = Math.max(0, resizingBlock.originalStart + deltaTime);
        const newEnd = resizingBlock.originalEnd;

        if (newEnd - newStart >= 200) {
          updateZoomBlock(resizingBlock.blockId, {
            start_time: newStart,
            end_time: newEnd,
          });
        }
      } else {
        const newStart = resizingBlock.originalStart;
        const newEnd = Math.min(duration, resizingBlock.originalEnd + deltaTime);

        if (newEnd - newStart >= 200) {
          updateZoomBlock(resizingBlock.blockId, {
            start_time: newStart,
            end_time: newEnd,
          });
        }
      }
    };

    const handleMouseUp = () => {
      setResizingBlock(null);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [resizingBlock, pixelsPerMs, duration, updateZoomBlock]);

  // Handle drag
  useEffect(() => {
    if (!draggingBlock) return;

    const handleMouseMove = (event: MouseEvent) => {
      const deltaX = event.clientX - draggingBlock.startX;
      const deltaTime = deltaX / pixelsPerMs;
      const blockDuration = draggingBlock.originalEnd - draggingBlock.originalStart;

      const newStart = Math.max(0, Math.min(
        duration - blockDuration,
        draggingBlock.originalStart + deltaTime
      ));
      const newEnd = newStart + blockDuration;

      updateZoomBlock(draggingBlock.blockId, {
        start_time: newStart,
        end_time: newEnd,
      });
    };

    const handleMouseUp = () => {
      setDraggingBlock(null);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [draggingBlock, pixelsPerMs, duration, updateZoomBlock]);

  return {
    draggingBlock,
    resizingBlock,
    handleDragStart,
    handleResizeStart,
  };
}
