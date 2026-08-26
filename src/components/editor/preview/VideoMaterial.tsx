import React, { useEffect, useRef, useState } from 'react';
import { useVideoTexture } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { isLinuxRuntime } from '../../../utils/platform';

interface VideoMaterialProps {
  videoUrl: string;
  isPlaying: boolean;
  cornerRadius?: number;
  aspectRatio?: number;
  cleanupWindowCorners?: boolean;
  suppressTimeUpdates?: boolean;
}

// Compatibility fallback for recordings made before native window silhouette
// sidecars were introduced.
const MACOS_WINDOW_CORNER_RADIUS_RATIO = 0.022;
const LINUX_NATIVE_VIDEO_READY_EVENT = 'tarantino-linux-native-video-ready';
const LINUX_PREVIEW_FPS = 30;
const LINUX_PREVIEW_BATCH_SIZE = 60;

const nativeVideoFrameHasPixels = (video: HTMLVideoElement): boolean => {
  if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA || video.videoWidth === 0) return false;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 32;
    canvas.height = 18;
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) return false;
    context.drawImage(video, 0, 0, canvas.width, canvas.height);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    for (let index = 0; index < pixels.length; index += 4) {
      if (pixels[index] > 2 || pixels[index + 1] > 2 || pixels[index + 2] > 2) return true;
    }
  } catch {
    return false;
  }
  return false;
};

export const VideoMaterial: React.FC<VideoMaterialProps> = ({
  videoUrl,
  isPlaying,
  cornerRadius = 0,
  aspectRatio = 16/9,
  cleanupWindowCorners = false,
  suppressTimeUpdates = false,
}) => {
  const texture = useVideoTexture(videoUrl, {
    unsuspend: 'loadedmetadata',
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

  useEffect(() => {
    if (!isLinuxRuntime || !videoElement) return;

    let announced = false;
    const announceUsableNativeFrame = () => {
      if (announced || !nativeVideoFrameHasPixels(videoElement)) return;
      announced = true;
      window.dispatchEvent(new Event(LINUX_NATIVE_VIDEO_READY_EVENT));
    };

    videoElement.addEventListener('loadeddata', announceUsableNativeFrame);
    videoElement.addEventListener('seeked', announceUsableNativeFrame);
    videoElement.addEventListener('timeupdate', announceUsableNativeFrame);
    announceUsableNativeFrame();

    return () => {
      videoElement.removeEventListener('loadeddata', announceUsableNativeFrame);
      videoElement.removeEventListener('seeked', announceUsableNativeFrame);
      videoElement.removeEventListener('timeupdate', announceUsableNativeFrame);
    };
  }, [videoElement]);

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
        if (!suppressTimeUpdates && !videoElement.paused && !videoElement.seeking) {
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
  }, [videoElement, duration, setDuration, setCurrentTime, suppressTimeUpdates]);

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

interface LinuxNativeVideoOverlayProps {
  isPlaying: boolean;
  onEnabledChange: (enabled: boolean) => void;
}

export const LinuxNativeVideoOverlay: React.FC<LinuxNativeVideoOverlayProps> = ({
  isPlaying,
  onEnabledChange,
}) => {
  const {
    currentTime,
    duration,
    videoFilePath,
    setCurrentTime,
    setIsPlaying,
  } = useEditorStore();
  const [enabled, setEnabled] = useState(false);
  const [canvasTexture, setCanvasTexture] = useState<THREE.CanvasTexture | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const requestFrameRef = useRef<((timeMs: number) => void) | null>(null);

  useEffect(() => {
    onEnabledChange(enabled);
    return () => onEnabledChange(false);
  }, [enabled, onEnabledChange]);

  useEffect(() => {
    let cancelled = false;
    let nativeReady = false;
    let vmFallbackRequired = false;
    let watchdog: number | undefined;

    const handleNativeReady = () => {
      nativeReady = true;
      if (!vmFallbackRequired) setEnabled(false);
    };
    const armFallback = () => {
      watchdog = window.setTimeout(() => {
        if (!cancelled && !nativeReady) setEnabled(true);
      }, 2000);
    };
    window.addEventListener(LINUX_NATIVE_VIDEO_READY_EVENT, handleNativeReady);

    void invoke<boolean>('linux_native_preview_required')
      .then((required) => {
        if (cancelled) return;
        vmFallbackRequired = required;
        if (required) {
          setEnabled(true);
          return;
        }
        armFallback();
      })
      .catch(armFallback);
    return () => {
      cancelled = true;
      if (watchdog !== undefined) window.clearTimeout(watchdog);
      window.removeEventListener(LINUX_NATIVE_VIDEO_READY_EVENT, handleNativeReady);
    };
  }, []);

  useEffect(() => {
    if (!enabled || !videoFilePath) return;

    let cancelled = false;
    let animationFrameId = 0;
    let inFlight = false;
    let queuedBucket: number | null = null;
    let lastRequestedBucket = -1;
    let drawRequest = 0;
    const frameCache = new Map<number, string>();
    const canvas = document.createElement('canvas');
    canvas.width = 2;
    canvas.height = 2;
    const context = canvas.getContext('2d', { alpha: false });
    if (!context) return;

    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.minFilter = THREE.LinearFilter;
    texture.magFilter = THREE.LinearFilter;
    setCanvasTexture(texture);

    const video = document.createElement('video');
    video.src = convertFileSrc(videoFilePath);
    video.muted = true;
    video.preload = 'metadata';
    video.playsInline = true;
    videoRef.current = video;

    const drawFrame = (dataUrl: string) => {
      const request = ++drawRequest;
      const image = new Image();
      image.onload = () => {
        if (cancelled || request !== drawRequest) return;
        canvas.width = Math.max(2, image.naturalWidth);
        canvas.height = Math.max(2, image.naturalHeight);
        context.drawImage(image, 0, 0, canvas.width, canvas.height);
        texture.needsUpdate = true;
      };
      image.src = dataUrl;
    };

    const requestBucket = async (bucket: number) => {
      if (cancelled || bucket === lastRequestedBucket) return;
      if (inFlight) {
        queuedBucket = bucket;
        return;
      }
      lastRequestedBucket = bucket;
      const cached = frameCache.get(bucket);
      if (cached) {
        drawFrame(cached);
        return;
      }
      inFlight = true;
      try {
        const batchStart = Math.floor(bucket / LINUX_PREVIEW_BATCH_SIZE) * LINUX_PREVIEW_BATCH_SIZE;
        const dataUrls = await invoke<string[]>('extract_video_preview_frames', {
          videoPath: videoFilePath,
          startBucket: batchStart,
          frameCount: LINUX_PREVIEW_BATCH_SIZE,
          previewWidth: 1280,
        });
        if (!cancelled) {
          dataUrls.forEach((dataUrl, index) => frameCache.set(batchStart + index, dataUrl));
          while (frameCache.size > 120) {
            const [oldestBucket] = frameCache.keys();
            frameCache.delete(oldestBucket);
          }
          const requestedFrame = frameCache.get(bucket);
          if (requestedFrame) drawFrame(requestedFrame);
        }
      } catch (error) {
        console.error('Linux native preview frame failed:', error);
      } finally {
        inFlight = false;
        if (queuedBucket !== null) {
          const nextBucket = queuedBucket;
          queuedBucket = null;
          void requestBucket(nextBucket);
        }
      }
    };

    requestFrameRef.current = (timeMs) => {
      void requestBucket(Math.max(0, Math.round(timeMs * LINUX_PREVIEW_FPS / 1000)));
    };
    requestFrameRef.current(currentTime);

    const playbackLoop = () => {
      if (cancelled) return;
      if (!video.paused) {
        const timeMs = video.currentTime * 1000;
        setCurrentTime(timeMs);
        requestFrameRef.current?.(timeMs);
      }
      animationFrameId = requestAnimationFrame(playbackLoop);
    };
    playbackLoop();

    const handleEnded = () => setIsPlaying(false);
    video.addEventListener('ended', handleEnded);

    return () => {
      cancelled = true;
      cancelAnimationFrame(animationFrameId);
      video.removeEventListener('ended', handleEnded);
      video.pause();
      video.removeAttribute('src');
      video.load();
      videoRef.current = null;
      requestFrameRef.current = null;
      texture.dispose();
      setCanvasTexture(null);
    };
  }, [enabled, videoFilePath, setCurrentTime, setIsPlaying]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (isPlaying) {
      video.currentTime = Math.min(currentTime / 1000, duration / 1000);
      video.play().catch((error) => console.error('Linux preview playback failed:', error));
    } else {
      video.pause();
    }
  }, [isPlaying, duration, enabled]);

  useEffect(() => {
    const video = videoRef.current;
    if (video && video.paused && Math.abs(video.currentTime * 1000 - currentTime) > 50) {
      video.currentTime = currentTime / 1000;
    }
    if (!isPlaying) requestFrameRef.current?.(currentTime);
  }, [currentTime, isPlaying]);

  if (!enabled || !canvasTexture) return null;
  return (
    <mesh position={[0, 0, 0.001]}>
      <planeGeometry args={[1, 1]} />
      <meshBasicMaterial map={canvasTexture} toneMapped={false} side={THREE.DoubleSide} />
    </mesh>
  );
};

export const VideoFallback: React.FC = () => (
  <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
);
