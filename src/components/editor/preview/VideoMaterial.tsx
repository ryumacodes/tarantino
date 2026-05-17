import React, { useEffect, useMemo, useRef } from 'react';
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

export const VideoMaterial: React.FC<VideoMaterialProps> = ({
  videoUrl,
  isPlaying,
  cornerRadius = 0,
  aspectRatio = 16/9,
  cleanupWindowCorners = false
}) => {
  const texture = useVideoTexture(videoUrl, {
    muted: true,
    loop: true,
    playsInline: true,
    crossOrigin: 'anonymous',
    start: true,
  });

  const audioRefs = useRef<HTMLAudioElement[]>([]);
  const { setCurrentTime, setDuration, duration, videoFilePath, hasMicrophone, hasSystemAudio, audioSettings } = useEditorStore();

  const alphaMask = useMemo(() => {
    if (cornerRadius <= 0 && !cleanupWindowCorners) return null;

    const maxSize = 512;
    const width = aspectRatio >= 1 ? maxSize : Math.max(2, Math.round(maxSize * aspectRatio));
    const height = aspectRatio >= 1 ? Math.max(2, Math.round(maxSize / aspectRatio)) : maxSize;
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return null;

    const radiusRatio = Math.max(cornerRadius / 100, cleanupWindowCorners ? 0.04 : 0);
    const radius = Math.min(width, height) * radiusRatio;

    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, width, height);
    ctx.fillStyle = '#fff';
    ctx.beginPath();
    ctx.roundRect(0, 0, width, height, radius);
    ctx.fill();

    const mask = new THREE.CanvasTexture(canvas);
    mask.colorSpace = THREE.NoColorSpace;
    mask.needsUpdate = true;
    return mask;
  }, [cornerRadius, aspectRatio, cleanupWindowCorners]);

  const videoElement = texture.image as HTMLVideoElement;

  // Expose video element globally for timeline control
  useEffect(() => {
    if (videoElement) {
      console.log('VideoMaterial: Exposing video element globally');
      (window as any).__TARANTINO_VIDEO_ELEMENT = videoElement;

      // Add global seek function for timeline integration
      (window as any).__TARANTINO_SEEK_VIDEO = (timeMs: number) => {
        if (videoElement) {
          videoElement.currentTime = timeMs / 1000;
        }
      };

      // Add global play/pause control (also syncs store)
      (window as any).__TARANTINO_SET_PLAYING = (playing: boolean) => {
        useEditorStore.getState().setIsPlaying(playing);
      };

      // Sync duration with store
      const handleLoadedMetadata = () => {
        const actualDuration = videoElement.duration * 1000;
        console.log('VideoMaterial: Video loaded, duration:', actualDuration);
        if (Math.abs(actualDuration - duration) > 1000) {
          console.log('VideoMaterial: Syncing duration to', actualDuration);
          setDuration(actualDuration);
        }
      };

      // Sync current time during playback
      const handleTimeUpdate = () => {
        if (!videoElement.paused && !videoElement.seeking) {
          setCurrentTime(videoElement.currentTime * 1000);
        }
      };

      videoElement.addEventListener('loadedmetadata', handleLoadedMetadata);
      videoElement.addEventListener('timeupdate', handleTimeUpdate);

      // Trigger metadata check if already loaded
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

  // Handle play/pause state
  useEffect(() => {
    if (videoElement) {
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
  }, [isPlaying, videoElement]);

  // Sync video currentTime with editor store
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

// Loading fallback component
export const VideoFallback: React.FC = () => (
  <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
);
