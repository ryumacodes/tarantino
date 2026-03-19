import React, { useEffect, useMemo } from 'react';
import { useVideoTexture } from '@react-three/drei';
import * as THREE from 'three';
import { useEditorStore } from '../../../stores/editor';
import { roundedCornersVertexShader, roundedCornersFragmentShader } from './shaders/roundedCorners.glsl';

interface VideoMaterialProps {
  videoUrl: string;
  isPlaying: boolean;
  cornerRadius?: number;
  aspectRatio?: number;
}

export const VideoMaterial: React.FC<VideoMaterialProps> = ({
  videoUrl,
  isPlaying,
  cornerRadius = 0,
  aspectRatio = 16/9
}) => {
  const texture = useVideoTexture(videoUrl, {
    muted: true,
    loop: true,
    playsInline: true,
    crossOrigin: 'anonymous',
    start: true,
  });

  const { setCurrentTime, setDuration, duration } = useEditorStore();

  // Rounded corners shader material
  const roundedShader = useMemo(() => {
    if (cornerRadius <= 0) return null;
    return {
      uniforms: {
        map: { value: texture },
        cornerRadius: { value: cornerRadius / 100 }, // 0-0.5 range
        aspectRatio: { value: aspectRatio },
      },
      vertexShader: roundedCornersVertexShader,
      fragmentShader: roundedCornersFragmentShader,
    };
  }, [texture, cornerRadius, aspectRatio]);

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

  // Handle play/pause state
  useEffect(() => {
    if (videoElement) {
      if (isPlaying) {
        videoElement.play().catch(err => console.error('Play failed:', err));
      } else {
        videoElement.pause();
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
      }
    }
  }, [currentTime, videoElement]);

  // Use rounded corner shader if cornerRadius > 0, otherwise use basic material
  if (roundedShader) {
    return <shaderMaterial args={[roundedShader]} side={THREE.DoubleSide} transparent />;
  }
  return <meshBasicMaterial map={texture} toneMapped={false} side={THREE.DoubleSide} />;
};

// Loading fallback component
export const VideoFallback: React.FC = () => (
  <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
);
