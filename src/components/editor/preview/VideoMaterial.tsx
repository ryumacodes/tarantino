import { useThree } from '@react-three/fiber';
import { observeDecodedFrame } from './previewRedrawEvents';
import { startPreviewMediaSync } from './previewMediaLoop';
import { useShallow } from 'zustand/react/shallow';
import { selectVideoMaterialInputs } from './hooks/videoInputs';
import {useMaskPreview} from './hooks/useMaskPreview';
import { subscribePreviewMute } from '../../../stores/previewAudio';
import type { SourceCrop } from '../../../utils/sourceCrop';
import { useSourceCropTexture } from './hooks/useSourceCropTexture';
import { resolveSourceCrop } from '../../../utils/sourceCrop';
import React, { useEffect, useRef, useState } from 'react';
import { useVideoTexture } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { createMediaSynchronizer } from '../../../utils/timeline/syncMedia';
import { createPreviewAudio, type PreviewAudioTrack } from '../../../utils/audio/previewAudio';
import { isLinuxRuntime } from '../../../utils/platform';

interface VideoMaterialProps {
  viewRef?: React.MutableRefObject<SourceCrop>;
  videoUrl: string;
  isPlaying: boolean;
  cornerRadius?: number;
  cornerRadiusRatio?: number;
  temporalMaskRef?: React.MutableRefObject<THREE.Texture|null>;
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
  cornerRadiusRatio,
  temporalMaskRef,
  aspectRatio = 16/9,
  cleanupWindowCorners = false,
  suppressTimeUpdates = false,
  viewRef,
}) => {
  const invalidate = useThree(state => state.invalidate);
  const texture = useVideoTexture(videoUrl, {
    unsuspend: 'loadedmetadata',
    muted: true,
    loop: false,
    playsInline: true,
    crossOrigin: 'anonymous',
    preload: 'auto',
    start: false,
  });

  const cropRect = useSourceCropTexture(texture,viewRef);
  const maskedMaterial=useMaskPreview(texture,viewRef);
  const audioRefs = useRef<HTMLAudioElement[]>([]);
  const audioGraph = useRef<ReturnType<typeof createPreviewAudio> | null>(null);
  const { setCurrentTime, setDuration, duration, videoFilePath, hasMicrophone, hasSystemAudio, audioSettings, gap } =
    useEditorStore(useShallow(selectVideoMaterialInputs));
  const [alphaMask, setAlphaMask] = useState<THREE.Texture | null>(null);
  const [warmedTexture, setWarmedTexture] = useState<THREE.VideoTexture | null>(null);
  const videoWarmedUp = warmedTexture === texture;

  useEffect(() => {
    let cancelled = false;
    let currentMask: THREE.CanvasTexture | null = null;
    let temporalMask: THREE.CanvasTexture | null = null;
    if(temporalMaskRef)temporalMaskRef.current=null;
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
        const radiusRatio = Math.max(cornerRadiusRatio ?? cornerRadius / 100, fallbackRadius);
        const radius = Math.min(width, height) * radiusRatio;
        ctx.fillStyle = '#fff';
        ctx.beginPath();
        ctx.roundRect(0, 0, width, height, radius);
        ctx.fill();
      }

      if(drawSilhouette && temporalMaskRef) {
        const raw=document.createElement('canvas');raw.width=width;raw.height=height;
        const rawContext=raw.getContext('2d');
        if(rawContext) {
          rawContext.drawImage(canvas,0,0);
          temporalMask=new THREE.CanvasTexture(raw);temporalMask.colorSpace=THREE.NoColorSpace;
          temporalMaskRef.current=temporalMask;
        }
      }
      if (drawSilhouette && cornerRadius > 0) {
        const radius = Math.min(width, height) * (cornerRadiusRatio ?? cornerRadius / 100);
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
        const maskCrop = cropRect ? resolveSourceCrop(cropRect,image.naturalWidth,image.naturalHeight)
          : {x:0,y:0,width:image.naturalWidth,height:image.naturalHeight};
        const nativeMaskScale = Math.min(
          1,
          1024 / maskCrop.width,
          1024 / maskCrop.height
        );
        const width = Math.max(2, Math.round(maskCrop.width * nativeMaskScale));
        const height = Math.max(2, Math.round(maskCrop.height * nativeMaskScale));
        publishMask(
          width,
          height,
          (ctx) => ctx.drawImage(image, maskCrop.x, maskCrop.y, maskCrop.width, maskCrop.height, 0, 0, width, height)
        );
      };
      image.onerror = fallback;
      image.src = convertFileSrc(maskPath);
    }

    return () => {
      cancelled = true;
      currentMask?.dispose();temporalMask?.dispose();
      if(temporalMaskRef)temporalMaskRef.current=null;
    };
  }, [cornerRadius, cornerRadiusRatio, temporalMaskRef, aspectRatio, cleanupWindowCorners, videoFilePath, cropRect]);

  const videoElement = texture.image as HTMLVideoElement;
  useEffect(() => {
    // A paused seek can finish after the previous texture upload. Refresh from
    // the decoded frame, including the initial seek back to the first frame.
    return observeDecodedFrame(videoElement, () => {
      texture.needsUpdate = true;
      invalidate(2);
    });
  }, [texture, videoElement, invalidate]);

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
      setWarmedTexture(null);
      try {
        // Subscribe before play resolves: WebKit can deliver its first frame
        // before the play promise settles.
        const firstFrame = waitForVideoFrame();
        await videoElement.play();
        await firstFrame;
      } catch (error) {
        console.warn('Video texture warm-up failed:', error);
      }

      if (cancelled) return;

      const playback = useEditorStore.getState();
      const targetTime = Math.max(
        0,
        Math.min((previewSample(playback, playback.currentTime).sourceTime ?? 0) / 1000, videoElement.duration || 0)
      );
      if (Number.isFinite(targetTime)) {
        videoElement.currentTime = targetTime;
      }
      texture.needsUpdate = true;

      if (!playback.isPlaying) {
        videoElement.pause();
      }
      setWarmedTexture(texture);
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
          useEditorStore.getState().setCurrentTime(timeMs);
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

      videoElement.addEventListener('loadedmetadata', handleLoadedMetadata);

      if (videoElement.readyState >= 1) {
        handleLoadedMetadata();
      }

      return () => {
        videoElement.removeEventListener('loadedmetadata', handleLoadedMetadata);
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

    let unsubscribeMute = () => {};
    if (audioRefs.current.length) {
      const tracks: PreviewAudioTrack[] = audioRefs.current.map((element, index) => ({
        element, kind: hasMicrophone && index === 0 ? 'microphone' : 'system',
      }));
      try {
        const graph = createPreviewAudio(tracks, useEditorStore.getState().audioSettings);
        audioGraph.current = graph;
        unsubscribeMute = subscribePreviewMute(muted => graph.setMuted(muted));
      }
      catch (error) {
        audioRefs.current.forEach(audio => { audio.muted = true; });
        console.error('Could not initialize audio preview processing:', error);
      }
    }
    return () => {
      unsubscribeMute();
      audioGraph.current?.dispose();
      audioGraph.current = null;
      audioRefs.current.forEach((audio) => {
        audio.pause();
        audio.src = '';
      });
      audioRefs.current = [];
    };
  }, [videoFilePath, hasMicrophone, hasSystemAudio]);

  useEffect(() => {
    audioGraph.current?.update(audioSettings);
  }, [audioSettings, hasMicrophone, hasSystemAudio, videoFilePath]);

  useEffect(() => {
    const updatePlayback = () => {
      void audioGraph.current?.setPlaying(useEditorStore.getState().isPlaying)
        .catch(error => console.error('Could not resume audio processing:', error));
    };
    updatePlayback();
    // Resume in the user's gesture as well as on state changes for autoplay policies.
    document.addEventListener('pointerdown', updatePlayback);
    document.addEventListener('keydown', updatePlayback);
    return () => {
      document.removeEventListener('pointerdown', updatePlayback);
      document.removeEventListener('keydown', updatePlayback);
    };
  }, [isPlaying, hasMicrophone, hasSystemAudio, videoFilePath]);

  useEffect(() => {
    if (!videoElement || !videoWarmedUp) return;
    const syncVideo = createMediaSynchronizer(videoElement, error => console.error('Preview playback failed:', error));
    const audioSync = new Map<HTMLAudioElement, ReturnType<typeof createMediaSynchronizer>>();
    const sync = () => {
      const state = useEditorStore.getState();
      const mapped = previewSample(state, state.currentTime);
      audioGraph.current?.setClipVolumes(mapped.micVolume, mapped.systemVolume);
      syncVideo(mapped, state.isPlaying, false, state.playbackRevision);
      for (const audio of audioRefs.current) {
        let controller = audioSync.get(audio);
        if (!controller) {
          controller = createMediaSynchronizer(audio, error => console.error('Audio preview failed:', error));
          audioSync.set(audio, controller);
        }
        controller(mapped, state.isPlaying, true, state.playbackRevision);
      }
    };
    const stop = startPreviewMediaSync(sync, [videoElement, ...audioRefs.current]);
    return () => { stop(); videoElement.pause(); audioRefs.current.forEach(a => a.pause()); };
  }, [videoElement, videoWarmedUp, suppressTimeUpdates, videoFilePath, hasMicrophone, hasSystemAudio]);

  const previewTexture = gap ? null : texture;
  const previewColor = gap ? "black" : "white";
  if (alphaMask) {
    return (
      <meshBasicMaterial
        ref={maskedMaterial}
        map={previewTexture}
        color={previewColor}
        alphaMap={alphaMask}
        transparent
        alphaTest={0.01}
        toneMapped={false}
        side={THREE.DoubleSide}
      />
    );
  }
  return <meshBasicMaterial ref={maskedMaterial} map={previewTexture} color={previewColor} toneMapped={false} side={THREE.DoubleSide} />;
};

interface LinuxNativeVideoOverlayProps {
  viewRef?: React.MutableRefObject<SourceCrop>;
  isPlaying: boolean;
  onEnabledChange: (enabled: boolean) => void;
}

export const LinuxNativeVideoOverlay: React.FC<LinuxNativeVideoOverlayProps> = ({
  isPlaying,
  onEnabledChange,
  viewRef,
}) => {
  const invalidate = useThree(state => state.invalidate);
  const {
    currentTime,
    duration,
    videoFilePath,
    setCurrentTime,
    setIsPlaying,
  } = useEditorStore();
  const [enabled, setEnabled] = useState(false);
  const [canvasTexture, setCanvasTexture] = useState<THREE.CanvasTexture | null>(null);
  useSourceCropTexture(canvasTexture,viewRef);
  const maskedMaterial=useMaskPreview(canvasTexture,viewRef);
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

    // Allocate the texture after decoding; GPU texture storage cannot grow.
    let texture: THREE.CanvasTexture | null = null;

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
        const width = Math.max(2, image.naturalWidth);
        const height = Math.max(2, image.naturalHeight);
        const resized = canvas.width !== width || canvas.height !== height;
        if (resized) {
          canvas.width = width;
          canvas.height = height;
        }
        context.drawImage(image, 0, 0, canvas.width, canvas.height);
        if (!texture || resized) {
          texture?.dispose();
          texture = new THREE.CanvasTexture(canvas);
          texture.colorSpace = THREE.SRGBColorSpace;
          texture.minFilter = THREE.LinearFilter;
          texture.magFilter = THREE.LinearFilter;
          setCanvasTexture(texture);
        }
        texture.needsUpdate = true;
        invalidate(2);
      };
      image.src = dataUrl;
    };

    const requestBucket = async (bucket: number, prefetchOnly = false) => {
      if (cancelled || (!prefetchOnly && bucket === lastRequestedBucket)) return;
      const cached = frameCache.get(bucket);
      if (cached) {
        if (!prefetchOnly) {
          lastRequestedBucket = bucket;
          drawFrame(cached);
          // Prefetch the next batch to avoid pauses at cache boundaries.
          const nextBatch = (Math.floor(bucket / LINUX_PREVIEW_BATCH_SIZE) + 1) * LINUX_PREVIEW_BATCH_SIZE;
          if (useEditorStore.getState().isPlaying && !inFlight
              && nextBatch < Math.ceil(duration * LINUX_PREVIEW_FPS / 1000)
              && !frameCache.has(nextBatch)) {
            void requestBucket(nextBatch, true);
          }
        }
        return;
      }
      if (inFlight) {
        if (!prefetchOnly) queuedBucket = bucket;
        return;
      }
      if (!prefetchOnly) lastRequestedBucket = bucket;
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
          if (requestedFrame && !prefetchOnly) drawFrame(requestedFrame);
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
      const lastBucket = Math.max(0, Math.ceil(duration * LINUX_PREVIEW_FPS / 1000) - 1);
      void requestBucket(Math.min(lastBucket, Math.max(0, Math.round(timeMs * LINUX_PREVIEW_FPS / 1000))));
    };
    requestFrameRef.current(previewSample(useEditorStore.getState(), currentTime).sourceTime ?? 0);

    const syncMedia = createMediaSynchronizer(video, error => console.error('Linux preview failed:', error));
    const playbackLoop = () => {
      if (cancelled) return;
      const state = useEditorStore.getState();
      const sample = previewSample(state, state.currentTime);
      syncMedia(sample, state.isPlaying);
      if (sample.sourceTime !== null) requestFrameRef.current?.(sample.sourceTime);
      animationFrameId = requestAnimationFrame(playbackLoop);
    };
    playbackLoop();

    return () => {
      cancelled = true;
      cancelAnimationFrame(animationFrameId);
      video.pause();
      video.removeAttribute('src');
      video.load();
      videoRef.current = null;
      requestFrameRef.current = null;
      texture?.dispose();
      setCanvasTexture(null);
    };
  }, [enabled, videoFilePath, duration, setCurrentTime, setIsPlaying, invalidate]);

  const gap = useEditorStore(state => previewSample(state, state.currentTime).sourceTime === null);
  if (!enabled || !canvasTexture || gap) return null;
  return (
    <mesh position={[0, 0, 0.001]}>
      <planeGeometry args={[1, 1]} />
      <meshBasicMaterial ref={maskedMaterial} map={canvasTexture} toneMapped={false} side={THREE.DoubleSide} />
    </mesh>
  );
};

export const VideoFallback: React.FC = () => (
  <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
);
