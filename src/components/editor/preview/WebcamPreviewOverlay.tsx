import React, { useRef, useEffect, useMemo, useState, useCallback } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';

interface WebcamPreviewOverlayProps {
  videoFilePath: string;
  corner: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
  size: number; // fraction of container width (0.08-0.25)
  shape: 'circle' | 'roundrect';
}

export const WebcamPreviewOverlay: React.FC<WebcamPreviewOverlayProps> = ({
  videoFilePath,
  corner,
  size,
  shape,
}) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hasSource, setHasSource] = useState(false);

  // Derive webcam.webm path from video file path
  // Pattern: recording.mp4 → recording.webcam.webm (alongside video)
  const webcamSrc = useMemo(() => {
    if (!videoFilePath) return null;
    const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
    let baseName = videoFilePath.split('/').pop()?.replace('.mp4', '') || '';
    // If video is processed_recording_*, the webcam.mp4 is at recording_* instead
    if (baseName.startsWith('processed_')) {
      baseName = baseName.replace('processed_', '');
    }
    const src = convertFileSrc(`${dir}/${baseName}.webcam.mp4`);
    console.log('[WebcamOverlay] videoFilePath:', videoFilePath, 'webcamSrc:', src);
    return src;
  }, [videoFilePath]);

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

  const margin = '3%';
  const positionStyle: React.CSSProperties = {
    position: 'absolute',
    width: `${size * 100}%`,
    aspectRatio: '1',
    zIndex: 10,
    pointerEvents: 'none',
    ...(corner.includes('top') ? { top: margin } : { bottom: margin }),
    ...(corner.includes('left') ? { left: margin } : { right: margin }),
  };

  const clipStyle: React.CSSProperties = {
    width: '100%',
    height: '100%',
    borderRadius: shape === 'circle' ? '50%' : '12%',
    overflow: 'hidden',
    boxShadow: '0 2px 12px rgba(0,0,0,0.4)',
    border: '2px solid rgba(255,255,255,0.15)',
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
          onError={() => setHasSource(false)}
          style={{
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            transform: 'scaleX(-1)',
            display: hasSource ? 'block' : 'none',
          }}
        />
      </div>
    </div>
  );
};
