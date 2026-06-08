import React, { useRef, useEffect, useMemo, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';

interface WebcamPreviewOverlayProps {
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
  x,
  y,
  size,
  shape,
}) => {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hasSource, setHasSource] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const updateVisualSettings = useEditorStore((state) => state.updateVisualSettings);

  const webcamSrcs = useMemo(() => {
    if (!videoFilePath) return [];
    const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
    let baseName = videoFilePath.split('/').pop()?.replace('.mp4', '') || '';
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

  useEffect(() => {
    setSrcIndex(0);
    setHasSource(false);
  }, [webcamSrcs]);

  useEffect(() => {
    if (!videoRef.current || !hasSource) return;
    const vid = videoRef.current;

    let rafId: number;

    const sync = () => {
      const mainVideo = window.__TARANTINO_VIDEO_ELEMENT;
      const currentTimeSec = mainVideo
        ? mainVideo.currentTime
        : useEditorStore.getState().currentTime / 1000;
      const isPlaying = mainVideo ? !mainVideo.paused : false;

      if (Math.abs(vid.currentTime - currentTimeSec) > 0.15) {
        vid.currentTime = currentTimeSec;
      }

      if (isPlaying && vid.paused) {
        vid.play().catch(() => {});
      } else if (!isPlaying && !vid.paused) {
        vid.pause();
      }

      rafId = requestAnimationFrame(sync);
    };

    rafId = requestAnimationFrame(sync);
    return () => cancelAnimationFrame(rafId);
  }, [hasSource]);

  if (!webcamSrc) return null;

  const clampedX = Math.min(1, Math.max(0, x));
  const clampedY = Math.min(1, Math.max(0, y));
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
    const closest = corners.reduce((nearest, corner) => {
      const nearestDistance = Math.hypot(nearest.x - nextX, nearest.y - nextY);
      const cornerDistance = Math.hypot(corner.x - nextX, corner.y - nextY);
      return cornerDistance < nearestDistance ? corner : nearest;
    }, { x: nextX, y: nextY });

    if (Math.hypot(closest.x - nextX, closest.y - nextY) < snapThreshold) {
      nextX = closest.x;
      nextY = closest.y;
    }

    updateVisualSettings({
      webcamX: nextX,
      webcamY: nextY,
      webcamCorner: nearestCorner(nextX, nextY),
    });
  };

  const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setIsDragging(true);
    moveToClientPoint(event.clientX, event.clientY);
  };

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (event: MouseEvent) => {
      moveToClientPoint(event.clientX, event.clientY);
    };
    const handleMouseUp = () => setIsDragging(false);

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, size, updateVisualSettings]);

  const positionStyle: React.CSSProperties = {
    position: 'absolute',
    width: `${size * 100}%`,
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
    borderRadius: shape === 'circle' ? '50%' : '12%',
    overflow: 'hidden',
    opacity: hasSource ? 1 : 0,
    boxShadow: hasSource ? '0 2px 12px rgba(0,0,0,0.4)' : 'none',
    border: hasSource ? '2px solid rgba(255,255,255,0.15)' : '2px solid transparent',
    background: hasSource ? '#000' : 'transparent',
    transition: isDragging ? 'none' : 'opacity 120ms ease, border-color 120ms ease, box-shadow 120ms ease',
  };

  return (
    <div
      ref={wrapperRef}
      style={positionStyle}
      onMouseDown={handleMouseDown}
      title="Drag webcam"
    >
      <div style={clipStyle}>
        <video
          ref={videoRef}
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
            transform: 'scaleX(-1)',
          }}
        />
      </div>
    </div>
  );
};
