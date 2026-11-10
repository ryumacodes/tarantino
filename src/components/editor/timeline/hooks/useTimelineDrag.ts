import { sourceTimeResolver, type SourceTimeResolver } from '../../../../utils/timeline/sourceTimeResolver';
import { useEditGesture } from '../../../../hooks/useEditGesture';
import { useEditorStore } from '../../../../stores/editor';
import { editSourceRange, zoomRangeLimits, type SourceRangeView } from '../../../../utils/timeline/sourceRanges';
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

  useEffect(() => {
    if (!isDragging || (dragType !== 'trim-start' && dragType !== 'trim-end')) return;
    const store = useEditorStore.getState();
    store.beginHistoryGroup();
    return () => store.endHistoryGroup();
  }, [isDragging, dragType]);

  const handleMouseDown = useCallback((e: React.MouseEvent, type: DragType) => {
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
      const x = e.clientX - rect.left + timelineRef.current.scrollLeft - trackHeaderWidth;
      const time = Math.max(0, Math.min(duration, x / pixelsPerMs));

      switch (dragType) {
        case 'playhead':
          setCurrentTime(time);
          seekVideo(time);

          const video = getVideoElement();
          if (video && !video.paused) {
            video.pause();
            window.__TARANTINO_WAS_PLAYING = true;
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
      if (dragType === 'playhead' && window.__TARANTINO_WAS_PLAYING) {
        const video = getVideoElement();
        if (video && video.paused) {
          video.play().catch(console.error);
        }
        window.__TARANTINO_WAS_PLAYING = false;
      }

      setIsDragging(false);
      setDragType(null);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    window.addEventListener('blur', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      window.removeEventListener('blur', handleMouseUp);
    };
  }, [isDragging, dragType, timelineRef, pixelsPerMs, duration, trackHeaderWidth, trimStart, trimEnd, setCurrentTime, setTrimStart, setTrimEnd, seekVideo, getVideoElement]);

  return { isDragging, dragType, handleMouseDown };
}

interface ZoomBlock extends SourceRangeView { id: string; zoomFactor: number; isManual: boolean; }
interface UseZoomBlockDragOptions {
  pixelsPerMs: number;
  duration: number;
  updateZoomBlock: (id: string, updates: { start_time?: number; end_time?: number }) => void;
  isExporting: boolean;
}
export function useZoomBlockDrag({ pixelsPerMs, duration, updateZoomBlock, isExporting }: UseZoomBlockDragOptions) {
  const gesture = useEditGesture();
  const [edit, setEdit] = useState<{ block: ZoomBlock; startX:number; type:'move'|'start'|'end'; limits:{start:number;end:number}; resolver:SourceTimeResolver | null } | null>(null);
  const begin = useCallback((event:React.MouseEvent, block:ZoomBlock, type:'move'|'start'|'end') => {
    if (isExporting) return;
    event.stopPropagation(); event.preventDefault();
    gesture.begin();
    const blocks = useEditorStore.getState().zoomAnalysis?.zoom_blocks ?? [];
    setEdit({block,startX:event.clientX,type,limits:zoomRangeLimits(block.id,blocks,duration),
      resolver:sourceTimeResolver(useEditorStore.getState(),block.startTime)});
  },[isExporting,gesture.begin,duration]);
  const handleDragStart=useCallback((event:React.MouseEvent,block:ZoomBlock) => {
    if ((event.target as HTMLElement).classList.contains('resize-handle')) return;
    begin(event,block,'move');
  },[begin]);
  const handleResizeStart=useCallback((event:React.MouseEvent,block:ZoomBlock,type:'start'|'end') => begin(event,block,type),[begin]);
  useEffect(() => {
    if (!edit) return;
    const move=(event:MouseEvent) => updateZoomBlock(edit.block.id,
      editSourceRange(edit.block,(event.clientX-edit.startX)/pixelsPerMs,edit.type,duration,edit.limits,edit.resolver));
    const stop=() => { gesture.end(); setEdit(null); };
    document.addEventListener('mousemove',move); document.addEventListener('mouseup',stop); window.addEventListener('blur',stop);
    return () => { document.removeEventListener('mousemove',move); document.removeEventListener('mouseup',stop); window.removeEventListener('blur',stop); };
  },[edit,pixelsPerMs,duration,updateZoomBlock,gesture.end]);
  return {handleDragStart,handleResizeStart};
}
