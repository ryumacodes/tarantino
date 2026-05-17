import React, { useRef, useEffect, useMemo, useState, useCallback } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';

interface WebcamPreviewOverlayProps {
  videoFilePath: string;
  corner: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
  x: number;
  y: number;
  size: number; // fraction of container width (0.08-0.25)
  shape: 'circle' | 'roundrect';
}

export const WebcamPreviewOverlay: React.FC<WebcamPreviewOverlayProps> = ({
  videoFilePath,
  x,
  y,
  size,
  shape,
}) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hasSource, setHasSource] = useState(false);

  // Derive webcam.webm path from video file path
  // Pattern: recording.mp4 → recording.webcam.webm (alongside video)
  const webcamSrcs = useMemo(() => {
    if (!videoFilePath) return [];
    const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
    let baseName = videoFilePath.split('/').pop()?.replace('.mp4', '') || '';
    // If video is processed_recording_*, the webcam.mp4 is at recording_* instead
    if (baseName.startsWith('processed_')) {
      baseName = baseName.replace('processed_', '');
    }
    const sources = [
      convertFileSrc(`${dir}/${baseName}.webcam.mp4`),
      convertFileSrc(`${dir}/${baseName}.webcam.webm`),
    ];
    console.log('[WebcamOverlay] videoFilePath:', videoFilePath, 'webcamSrcs:', sources);
    return sources;
  }, [videoFilePath]);
  const [srcIndex, setSrcIndex] = useState(0);
  const webcamSrc = webcamSrcs[srcIndex] ?? null;

  useEffect(() => {
    setSrcIndex(0);
    setHasSource(false);
  }, [webcamSrcs]);

  useEffect(() => {
    console.log('[WebcamOverlay] hasSource:', hasSource, 'webcamSrc:', webcamSrc);
  }, [hasSource, webcamSrc]);

  // Sync webcam video with main video using editor store time
  useEffect(() => {
    if (!videoRef.current || !hasSource) return;
    const vid = videoRef.current;

    let rafId: number;
    let lastTime = -1;

    const sync = () => {
      // Read current time from the main video element or editor store
      const mainVideo = (window as any).__TARANTINO_VIDEO_ELEMENT as HTMLVideoElement | undefined;
      const currentTimeSec = mainVideo
        ? mainVideo.currentTime
        : useEditorStore.getState().currentTime / 1000;
      const isPlaying = mainVideo ? !mainVideo.paused : false;

      // Seek webcam to match
      if (Math.abs(vid.currentTime - currentTimeSec) > 0.15) {
        vid.currentTime = currentTimeSec;
      }

      // Match play/pause state
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
  const positionStyle: React.CSSProperties = {
    position: 'absolute',
    width: `${size * 100}%`,
    aspectRatio: '1',
    zIndex: 10,
    pointerEvents: 'none',
    left: `${clampedX * 100}%`,
    top: `${clampedY * 100}%`,
    transform: 'translate(-50%, -50%)',
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
    transition: 'opacity 120ms ease',
  };

  return (
    <div style={positionStyle}>
      <div style={clipStyle}>
        <video
          ref={videoRef}
          src={webcamSrc}
          muted
          playsInline
          onLoadedData={() => setHasSource(true)}
          onCanPlay={() => setHasSource(true)}
          onError={(event) => {
            console.warn('[WebcamOverlay] failed to load webcam video', webcamSrc, event);
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
