import React, { useEffect, useRef, useState } from 'react';
import { useVideoTexture } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';

interface VideoMaterialProps {
  videoUrl: string;
  isPlaying: boolean;
  cornerRadius?: number;
  aspectRatio?: number;
  cleanupWindowCorners?: boolean;
}

// Compatibility fallback for recordings made before native window silhouette
// sidecars were introduced.
const MACOS_WINDOW_CORNER_RADIUS_RATIO = 0.022;

export const VideoMaterial: React.FC<VideoMaterialProps> = ({
  videoUrl,
  isPlaying,
  cornerRadius = 0,
  aspectRatio = 16/9,
  cleanupWindowCorners = false
}) => {
  const texture = useVideoTexture(videoUrl, {
    unsuspend: 'loadeddata',
    muted: true,
    loop: true,
    playsInline: true,
    crossOrigin: 'anonymous',
    preload: 'auto',
    start: true,
  });

  const audioRefs = useRef<HTMLAudioElement[]>([]);
  const { setCurrentTime, setDuration, duration, videoFilePath, hasMicrophone, hasSystemAudio, audioSettings } = useEditorStore();
  const [alphaMask, setAlphaMask] = useState<THREE.Texture | null>(null);
  const [videoWarmedUp, setVideoWarmedUp] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let currentMask: THREE.CanvasTexture | null = null;
    const maxSize = 512;
    const publishMask = (
      width: number,
      height: number,
      drawSilhouette?: (ctx: CanvasRenderingContext2D) => void,
      fallbackRadius = 0
    ) => {
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext('2d');
      if (!ctx || cancelled) return;

      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, width, height);
      if (drawSilhouette) {
        drawSilhouette(ctx);
      } else {
        const radiusRatio = Math.max(cornerRadius / 100, fallbackRadius);
        const radius = Math.min(width, height) * radiusRatio;
        ctx.fillStyle = '#fff';
        ctx.beginPath();
        ctx.roundRect(0, 0, width, height, radius);
        ctx.fill();
      }

      if (drawSilhouette && cornerRadius > 0) {
        const radius = Math.min(width, height) * (cornerRadius / 100);
        ctx.globalCompositeOperation = 'destination-in';
        ctx.fillStyle = '#fff';
        ctx.beginPath();
        ctx.roundRect(0, 0, width, height, radius);
        ctx.fill();
      }

      currentMask = new THREE.CanvasTexture(canvas);
      currentMask.colorSpace = THREE.NoColorSpace;
      currentMask.needsUpdate = true;
      setAlphaMask(currentMask);
    };

    const fallback = () => {
      if (!cleanupWindowCorners && cornerRadius <= 0) {
        setAlphaMask(null);
        return;
      }
      const width = aspectRatio >= 1
        ? maxSize
        : Math.max(2, Math.round(maxSize * aspectRatio));
      const height = aspectRatio >= 1
        ? Math.max(2, Math.round(maxSize / aspectRatio))
        : maxSize;
      publishMask(
        width,
        height,
        undefined,
        cleanupWindowCorners ? MACOS_WINDOW_CORNER_RADIUS_RATIO : 0
      );
    };

    if (!cleanupWindowCorners || !videoFilePath) {
      fallback();
    } else {
      const maskPath = `${videoFilePath.replace(/\.[^/.]+$/, '')}.window-mask.png`;
      const image = new Image();
      image.crossOrigin = 'anonymous';
      image.onload = () => {
        const nativeMaskScale = Math.min(
          1,
          1024 / image.naturalWidth,
          1024 / image.naturalHeight
        );
        const width = Math.max(2, Math.round(image.naturalWidth * nativeMaskScale));
        const height = Math.max(2, Math.round(image.naturalHeight * nativeMaskScale));
        publishMask(
          width,
          height,
          (ctx) => ctx.drawImage(image, 0, 0, width, height)
        );
      };
      image.onerror = fallback;
      image.src = convertFileSrc(maskPath);
    }

    return () => {
      cancelled = true;
      currentMask?.dispose();
    };
  }, [cornerRadius, aspectRatio, cleanupWindowCorners, videoFilePath]);

  const videoElement = texture.image as HTMLVideoElement;

  // WKWebView can leave a paused HTML video as a permanently black GPU
  // texture if it is paused before WebKit has submitted its first frame.
  // Briefly play through one frame before applying the editor's play state.
  useEffect(() => {
    if (!videoElement) return;

    let cancelled = false;
    let frameCallbackId: number | null = null;

    const waitForVideoFrame = () => new Promise<void>((resolve) => {
      if (typeof videoElement.requestVideoFrameCallback === 'function') {
        frameCallbackId = videoElement.requestVideoFrameCallback(() => resolve());
      } else {
        window.setTimeout(resolve, 0);
      }
    });

    const warmUpVideoTexture = async () => {
      setVideoWarmedUp(false);
      try {
        await videoElement.play();
        await waitForVideoFrame();
      } catch (error) {
        // Playback can still be rejected by WebKit in unusual system states;
        // allow normal playback controls to retry instead of blocking them.
        console.warn('Video texture warm-up failed:', error);
      }

      if (cancelled) return;

      const playback = useEditorStore.getState();
      const targetTime = Math.max(
        0,
        Math.min(playback.currentTime / 1000, videoElement.duration || 0)
      );
      if (Number.isFinite(targetTime)) {
        videoElement.currentTime = targetTime;
      }
      texture.needsUpdate = true;

      if (!playback.isPlaying) {
        videoElement.pause();
      }
      setVideoWarmedUp(true);
    };

    void warmUpVideoTexture();

    return () => {
      cancelled = true;
      if (frameCallbackId !== null && typeof videoElement.cancelVideoFrameCallback === 'function') {
        videoElement.cancelVideoFrameCallback(frameCallbackId);
      }
    };
  }, [texture, videoElement]);

  useEffect(() => {
    if (videoElement) {
      window.__TARANTINO_VIDEO_ELEMENT = videoElement;

      window.__TARANTINO_SEEK_VIDEO = (timeMs: number) => {
        if (videoElement) {
          videoElement.currentTime = timeMs / 1000;
        }
      };

      window.__TARANTINO_SET_PLAYING = (playing: boolean) => {
        useEditorStore.getState().setIsPlaying(playing);
      };

      const handleLoadedMetadata = () => {
        const actualDuration = videoElement.duration * 1000;
        if (Math.abs(actualDuration - duration) > 1000) {
          setDuration(actualDuration);
        }
      };

      const handleTimeUpdate = () => {
        if (!videoElement.paused && !videoElement.seeking) {
          setCurrentTime(videoElement.currentTime * 1000);
        }
      };

      videoElement.addEventListener('loadedmetadata', handleLoadedMetadata);
      videoElement.addEventListener('timeupdate', handleTimeUpdate);

      if (videoElement.readyState >= 1) {
        handleLoadedMetadata();
      }

      return () => {
        videoElement.removeEventListener('loadedmetadata', handleLoadedMetadata);
        videoElement.removeEventListener('timeupdate', handleTimeUpdate);
      };
    }
  }, [videoElement, duration, setDuration, setCurrentTime]);

  useEffect(() => {
    audioRefs.current.forEach((audio) => {
      audio.pause();
      audio.src = '';
    });

    if (!videoFilePath) {
      audioRefs.current = [];
      return;
    }

    const basePath = videoFilePath.replace(/\.[^/.]+$/, '');
    const paths: string[] = [];
    if (hasMicrophone) paths.push(`${basePath}.mic.wav`);
    if (hasSystemAudio) paths.push(`${basePath}.system.wav`);

    audioRefs.current = paths.map((path) => {
      const audio = new Audio(convertFileSrc(path));
      audio.preload = 'auto';
      audio.crossOrigin = 'anonymous';
      return audio;
    });

    return () => {
      audioRefs.current.forEach((audio) => {
        audio.pause();
        audio.src = '';
      });
      audioRefs.current = [];
    };
  }, [videoFilePath, hasMicrophone, hasSystemAudio]);

  useEffect(() => {
    const toVolume = (db: number) => Math.max(0, Math.min(4, Math.pow(10, db / 20)));
    audioRefs.current.forEach((audio, index) => {
      if (hasMicrophone && index === 0) {
        audio.volume = Math.min(1, toVolume(audioSettings.micGain));
      } else {
        audio.volume = Math.min(1, toVolume(audioSettings.systemGain));
      }
    });
  }, [audioSettings.micGain, audioSettings.systemGain, hasMicrophone]);

  useEffect(() => {
    if (videoElement && videoWarmedUp) {
      if (isPlaying) {
        audioRefs.current.forEach((audio) => {
          audio.currentTime = videoElement.currentTime;
          audio.play().catch(() => {});
        });
        videoElement.play().catch(err => console.error('Play failed:', err));
      } else {
        videoElement.pause();
        audioRefs.current.forEach((audio) => audio.pause());
      }
    }
  }, [isPlaying, videoElement, videoWarmedUp]);

  const { currentTime } = useEditorStore();
  useEffect(() => {
    if (videoElement && videoElement.paused) {
      const videoTime = currentTime / 1000;
      if (Math.abs(videoElement.currentTime - videoTime) > 0.05) {
        videoElement.currentTime = videoTime;
        audioRefs.current.forEach((audio) => {
          audio.currentTime = videoTime;
        });
      }
    }
  }, [currentTime, videoElement]);

  if (alphaMask) {
    return (
      <meshBasicMaterial
        map={texture}
        alphaMap={alphaMask}
        transparent
        alphaTest={0.01}
        toneMapped={false}
        side={THREE.DoubleSide}
      />
    );
  }
  return <meshBasicMaterial map={texture} toneMapped={false} side={THREE.DoubleSide} />;
};

export const VideoFallback: React.FC = () => (
  <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
);
