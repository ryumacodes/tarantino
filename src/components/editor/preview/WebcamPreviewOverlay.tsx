import { useCameraSilence } from '../../../hooks/useCameraSilence';
import { useEditGesture } from '../../../hooks/useEditGesture';
import { snapWebcam } from '../../../utils/camera/snapWebcam';
import { containWebcam } from '../../../utils/camera/containWebcam';
import { sampleWebcam } from '../../../utils/camera/webcamTimeline';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { createMediaSynchronizer } from '../../../utils/timeline/syncMedia';
import React, { useRef, useEffect, useMemo, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';

interface WebcamPreviewOverlayProps {
  gpuVideo?: (video:HTMLVideoElement|null)=>void;
  videoFilePath: string;
  x: number;
  y: number;
  size: number;
  shape: 'circle' | 'roundrect';
}

type WebcamCorner = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

const nearestCorner = (x: number, y: number): WebcamCorner => {
  const horizontal = x < 0.5 ? 'left' : 'right';
  const vertical = y < 0.5 ? 'top' : 'bottom';
  return `${vertical}-${horizontal}` as WebcamCorner;
};

export const WebcamPreviewOverlay: React.FC<WebcamPreviewOverlayProps> = ({
  videoFilePath,
  gpuVideo,
  x: _x,
  y: _y,
  size: _size,
  shape: _shape,
}) => {
  const gesture = useEditGesture();
  const state = useEditorStore();
  const source = previewSample(state, state.currentTime);
  const silence=useCameraSilence();
  const camera = sampleWebcam(state.visualSettings, state.webcamKeyframes, source.sourceTime ?? 0, silence.ranges);
  const {x,y,size,shape,layout} = camera;
  const wrapperRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hasSource, setHasSource] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const updateVisualSettings = useEditorStore((state) => state.updateVisualSettings);
  const gap = useEditorStore(state => previewSample(state, state.currentTime).sourceTime === null);

  const webcamSrcs = useMemo(() => {
    if (!videoFilePath) return [];
    const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
    let baseName = videoFilePath.split('/').pop()?.replace(/\.[^/.]+$/, '') || '';
    if (baseName.startsWith('processed_')) {
      baseName = baseName.replace('processed_', '');
    }
    return [
      convertFileSrc(`${dir}/${baseName}.webcam.mp4`),
      convertFileSrc(`${dir}/${baseName}.webcam.webm`),
    ];
  }, [videoFilePath]);
  const [srcIndex, setSrcIndex] = useState(0);
  const webcamSrc = webcamSrcs[srcIndex] ?? null;
  useEffect(()=>{gpuVideo?.(hasSource?videoRef.current:null);return ()=>gpuVideo?.(null);},[gpuVideo,hasSource,webcamSrc]);

  useEffect(() => {
    setSrcIndex(0);
    setHasSource(false);
  }, [webcamSrcs]);

  useEffect(() => {
    if (!videoRef.current || !hasSource) return;
    const vid = videoRef.current;

    let rafId: number;

    const syncMedia = createMediaSynchronizer(vid, error => console.error('Webcam preview failed:', error));
    const sync = () => {
      const state = useEditorStore.getState();
      syncMedia(previewSample(state, state.currentTime), state.isPlaying);

      rafId = requestAnimationFrame(sync);
    };

    rafId = requestAnimationFrame(sync);
    return () => cancelAnimationFrame(rafId);
  }, [hasSource]);


  const [canvasSize, setCanvasSize] = useState({ width: 1, height: 1 });
  useEffect(() => {
    const parent = wrapperRef.current?.parentElement;
    if (!parent) return;
    const observer = new ResizeObserver(([entry]) => {
      setCanvasSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(parent);
    return () => observer.disconnect();
  }, []);
  const contained = containWebcam(x, y, size, canvasSize.width, canvasSize.height);
  const clampedX = contained.x;
  const clampedY = contained.y;
  const moveToClientPoint = (clientX: number, clientY: number) => {
    const parent = wrapperRef.current?.parentElement;
    if (!parent) return;

    const rect = parent.getBoundingClientRect();
    const box = wrapperRef.current?.getBoundingClientRect();
    const halfX = box ? box.width / rect.width / 2 : size / 2;
    const halfY = box ? box.height / rect.height / 2 : size / 2;
    const margin = 10;
    const marginX = Math.min(0.08, margin / rect.width);
    const marginY = Math.min(0.08, margin / rect.height);
    const nearX = Math.min(0.5, halfX + marginX);
    const farX = Math.max(0.5, 1 - halfX - marginX);
    const nearY = Math.min(0.5, halfY + marginY);
    const farY = Math.max(0.5, 1 - halfY - marginY);
    const rawX = (clientX - rect.left) / rect.width;
    const rawY = (clientY - rect.top) / rect.height;
    let nextX = Math.max(nearX, Math.min(farX, rawX));
    let nextY = Math.max(nearY, Math.min(farY, rawY));

    const snapThreshold = 0.09;
    const corners = [
      { x: nearX, y: nearY },
      { x: farX, y: nearY },
      { x: nearX, y: farY },
      { x: farX, y: farY },
    ];
    const snapped = snapWebcam({ x: nextX, y: nextY }, corners, snapThreshold);
    nextX = snapped.x;
    nextY = snapped.y;

    if (camera.keyframeId) {
      state.updateWebcamKeyframe(camera.keyframeId, {x:nextX,y:nextY});
      return;
    }
    updateVisualSettings({
      webcamX: nextX,
      webcamY: nextY,
      webcamCorner: nearestCorner(nextX, nextY),
    });
  };

  const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    if (layout === 'fullscreen') return;
    event.preventDefault();
    event.stopPropagation();
    gesture.begin();
    setIsDragging(true);
    moveToClientPoint(event.clientX, event.clientY);
  };

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (event: MouseEvent) => {
      moveToClientPoint(event.clientX, event.clientY);
    };
    const handleMouseUp = () => { gesture.end(); setIsDragging(false); };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    window.addEventListener('blur', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      window.removeEventListener('blur', handleMouseUp);
    };
  }, [isDragging, size, updateVisualSettings, camera.keyframeId, layout, gesture.end]);

  const positionStyle: React.CSSProperties = {
    position: 'absolute',
    visibility: gap || !camera.visible ? 'hidden' : 'visible',
    width: `${contained.size * 100}%`,
    aspectRatio: '1',
    zIndex: 10,
    pointerEvents: 'auto',
    left: `${clampedX * 100}%`,
    top: `${clampedY * 100}%`,
    transform: 'translate(-50%, -50%)',
    cursor: isDragging ? 'grabbing' : 'grab',
    userSelect: 'none',
    touchAction: 'none',
  };

  const clipStyle: React.CSSProperties = {
    width: '100%',
    height: '100%',
    borderRadius: layout === 'fullscreen' ? 0 : shape === 'circle' ? '50%' : '12%',
    overflow: 'hidden',
    opacity: hasSource && !gpuVideo ? 1 : 0,
    boxShadow: hasSource && layout !== 'fullscreen' ? '0 2px 12px rgba(0,0,0,0.4)' : 'none',
    border: layout === 'fullscreen' ? 'none' : hasSource ? '2px solid rgba(255,255,255,0.15)' : '2px solid transparent',
    background: hasSource ? '#000' : 'transparent',
    transition: isDragging ? 'none' : 'opacity 120ms ease, border-color 120ms ease, box-shadow 120ms ease',
  };

  if (!webcamSrc) return null;
  return (
    <div
      ref={wrapperRef}
      style={layout === 'fullscreen' ? {...positionStyle,width:'100%',height:'100%',left:0,top:0,transform:'none',aspectRatio:'auto'} : positionStyle}
      onMouseDown={handleMouseDown}
      title="Drag webcam"
    >
      <div style={clipStyle}>
        <video
          ref={videoRef}
          crossOrigin="anonymous"
          src={webcamSrc}
          muted
          playsInline
          onLoadedData={() => setHasSource(true)}
          onCanPlay={() => setHasSource(true)}
          onError={() => {
            if (srcIndex < webcamSrcs.length - 1) {
              setSrcIndex((index) => index + 1);
              return;
            }
            setHasSource(false);
          }}
          style={{
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            transform: state.visualSettings.webcamMirrored ? 'scaleX(-1)' : undefined,
          }}
        />
      </div>
    </div>
  );
};
