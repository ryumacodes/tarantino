import React, { useRef, useState, useEffect, Suspense } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import { Text } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore, SPRING_PRESETS } from '../../../stores/editor';
import { VideoMaterial, VideoFallback } from './VideoMaterial';
import { BackgroundPlane } from './BackgroundPlane';
import { VideoShadow } from './VideoShadow';

interface SpringConfig {
  tension: number;
  friction: number;
  mass: number;
}

interface SpringState {
  value: number;
  velocity: number;
}

const springStep = (
  current: number,
  target: number,
  velocity: number,
  config: SpringConfig,
  dt: number
): SpringState => {
  const { tension, friction, mass } = config;
  const safeDt = Math.min(dt, 0.064);

  const displacement = current - target;
  const springForce = -tension * displacement;
  const dampingForce = -friction * velocity;
  const acceleration = (springForce + dampingForce) / mass;

  const newVelocity = velocity + acceleration * safeDt;
  const newValue = current + newVelocity * safeDt;

  const isSettled = Math.abs(displacement) < 0.0001 && Math.abs(newVelocity) < 0.0001;
  if (isSettled) {
    return { value: target, velocity: 0 };
  }

  return { value: newValue, velocity: newVelocity };
};

interface VideoTransform {
  scale: number;
  offsetX: number;
  offsetY: number;
  viewportWidth: number;
  viewportHeight: number;
  planeWidth: number;
  planeHeight: number;
}

interface VideoViewerProps {
  showMouseOverlay: boolean;
  isPlaying: boolean;
  velocityRef: React.MutableRefObject<{ scale: number; x: number; y: number }>;
  videoTransformRef: React.MutableRefObject<VideoTransform>;
  previewZoom?: number;
}

const WINDOW_EDGE_DETAIL_MARGIN = 0.22;

const clamp01 = (value: number): number => Math.max(0, Math.min(1, value));

const smoothstep = (value: number): number => {
  const t = clamp01(value);
  return t * t * (3 - 2 * t);
};

const getWindowZoomAnchor = (value: number, scale: number): number => {
  const edgeAnchor = Math.max(
    WINDOW_EDGE_DETAIL_MARGIN,
    Math.min(1 - WINDOW_EDGE_DETAIL_MARGIN, value)
  );
  const strength = smoothstep((scale - 1) / 1.0);
  return value + (edgeAnchor - value) * strength;
};

export const VideoViewer: React.FC<VideoViewerProps> = ({
  showMouseOverlay,
  isPlaying,
  velocityRef,
  videoTransformRef,
  previewZoom = 1
}) => {
  const { videoFilePath, zoomAnalysis, currentTime, loadMouseEvents, visualSettings, displayResolution, videoWidth, videoHeight, captureMode } = useEditorStore();
  const meshRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);
  const { viewport } = useThree();

  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoError, setVideoError] = useState<string | null>(null);

  useEffect(() => {
    const loadVideo = async () => {
      if (videoFilePath) {
        try {
          const url = convertFileSrc(videoFilePath);
          setVideoUrl(url);
          setVideoError(null);

          const mouseFilePath = videoFilePath.replace('.mp4', '.mouse.json');
          loadMouseEvents(mouseFilePath);
        } catch (err) {
          console.error('VideoViewer: Failed to convert file path:', err);
          setVideoError(`Failed to load video: ${err}`);
        }
      }
    };
    loadVideo();
  }, [videoFilePath, loadMouseEvents]);

  const ASPECT_MAP: Record<string, number> = {
    '16:9': 16/9, '9:16': 9/16, '4:3': 4/3, '1:1': 1, '21:9': 21/9,
  };
  const sourceAspect = (videoWidth && videoHeight)
    ? videoWidth / videoHeight
    : (displayResolution ? displayResolution.width / displayResolution.height : 16 / 9);
  const isWindowFocus = captureMode === 'window' && visualSettings.windowLayoutMode === 'focus';
  const videoAspect = isWindowFocus
    ? sourceAspect
    : captureMode === 'window'
      ? (ASPECT_MAP[visualSettings.aspectRatio] || sourceAspect)
      : (displayResolution ? displayResolution.width / displayResolution.height : 16 / 9);
  const viewportAspect = viewport.width / viewport.height;

  let basePlaneWidth: number, basePlaneHeight: number;
  const fitW = viewport.width * previewZoom;
  const fitH = viewport.height * previewZoom;
  if (videoAspect > viewportAspect) {
    basePlaneWidth = fitW;
    basePlaneHeight = fitW / videoAspect;
  } else {
    basePlaneHeight = fitH;
    basePlaneWidth = fitH * videoAspect;
  }
  if (basePlaneWidth > fitW) {
    basePlaneHeight *= fitW / basePlaneWidth;
    basePlaneWidth = fitW;
  }
  if (basePlaneHeight > fitH) {
    basePlaneWidth *= fitH / basePlaneHeight;
    basePlaneHeight = fitH;
  }

  let planeWidth: number, planeHeight: number;

  if (captureMode === 'window') {
    const inset = Math.max(0.01, 1 - 2 * (visualSettings.padding / 100));
    const contentW = basePlaneWidth * inset;
    const contentH = basePlaneHeight * inset;
    if (isWindowFocus) {
      planeWidth = contentW;
      planeHeight = contentH;
    } else {
      if (sourceAspect > videoAspect) {
        planeWidth = contentW;
        planeHeight = contentW / sourceAspect;
      } else {
        planeHeight = contentH;
        planeWidth = contentH * sourceAspect;
      }
    }
  } else {
    const paddingFactor = 1 - 2 * (visualSettings.padding / 100);
    planeWidth = basePlaneWidth * Math.max(0.01, paddingFactor);
    planeHeight = basePlaneHeight * Math.max(0.01, paddingFactor);
  }

  const { getCursorAtTime } = useEditorStore();

  const zoomSpring = useRef<SpringState>({ value: 1, velocity: 0 });
  const cursorSpringX = useRef<SpringState>({ value: 0.5, velocity: 0 });
  const cursorSpringY = useRef<SpringState>({ value: 0.5, velocity: 0 });
  const prevActiveBlockRef = useRef<string | null>(null);
  const lastBlockOutConfigRef = useRef<SpringConfig | null>(null);

  const globalZoomConfig = SPRING_PRESETS[visualSettings.zoomSpeedPreset];
  const cursorConfig = SPRING_PRESETS[visualSettings.cursorSpeedPreset];

  const zoomPanConfig = {
    tension: 80,
    friction: 40,
    mass: 2.0
  };

  const followPanConfig = {
    tension: 520,
    friction: 52,
    mass: 1.0
  };

  useFrame((state, delta) => {
    if (!meshRef.current) return;

    let targetScale = 1;
    let targetCenterX = 0.5;
    let targetCenterY = 0.5;
    let isZooming = false;
    let isFollowPhase = false;
    let zoomSpringConfig: SpringConfig = lastBlockOutConfigRef.current ?? globalZoomConfig;

    if (zoomAnalysis && zoomAnalysis.zoom_blocks.length > 0) {
      const activeBlock = zoomAnalysis.zoom_blocks.find(
        block => currentTime >= block.start_time && currentTime <= block.end_time
      );

      if (activeBlock) {
        isZooming = true;
        targetScale = activeBlock.zoom_factor;

        const blockInConfig = activeBlock.zoom_in_speed
          ? SPRING_PRESETS[activeBlock.zoom_in_speed] : globalZoomConfig;
        const blockOutConfig = activeBlock.zoom_out_speed
          ? SPRING_PRESETS[activeBlock.zoom_out_speed] : globalZoomConfig;

        const blockKey = `${activeBlock.start_time}-${activeBlock.end_time}`;
        if (prevActiveBlockRef.current !== blockKey) {
          prevActiveBlockRef.current = blockKey;
          const alreadyZoomed = zoomSpring.current.value > 1.1;
          if (!alreadyZoomed) {
            cursorSpringX.current = { value: activeBlock.center_x, velocity: 0 };
            cursorSpringY.current = { value: activeBlock.center_y, velocity: 0 };
          }
        }

        zoomSpringConfig = blockInConfig;
        const cursorPos = getCursorAtTime(currentTime);
        const centers = activeBlock.centers ?? [];
        const firstCenterTime = centers.length > 0
          ? Math.min(...centers.map(center => center.time))
          : activeBlock.start_time;

        let anchorCenterX = activeBlock.center_x;
        let anchorCenterY = activeBlock.center_y;
        for (const center of centers) {
          if (currentTime >= center.time) {
            anchorCenterX = center.x;
            anchorCenterY = center.y;
          }
        }

        if (currentTime >= firstCenterTime && cursorPos) {
          isFollowPhase = true;
          targetCenterX = cursorPos.x;
          targetCenterY = cursorPos.y;
        } else {
          targetCenterX = anchorCenterX;
          targetCenterY = anchorCenterY;
        }

        lastBlockOutConfigRef.current = blockOutConfig;
      }
    }

    if (!isZooming) {
      targetCenterX = 0.5;
      targetCenterY = 0.5;
      prevActiveBlockRef.current = null;
    }

    targetCenterX = Math.max(0.0, Math.min(1.0, targetCenterX));
    targetCenterY = Math.max(0.0, Math.min(1.0, targetCenterY));

    const panSpringConfig = isFollowPhase ? followPanConfig : (isZooming ? zoomPanConfig : cursorConfig);
    cursorSpringX.current = springStep(
      cursorSpringX.current.value,
      targetCenterX,
      cursorSpringX.current.velocity,
      panSpringConfig,
      delta
    );
    cursorSpringY.current = springStep(
      cursorSpringY.current.value,
      targetCenterY,
      cursorSpringY.current.velocity,
      panSpringConfig,
      delta
    );

    zoomSpring.current = springStep(
      zoomSpring.current.value,
      targetScale,
      zoomSpring.current.velocity,
      zoomSpringConfig,
      delta
    );

    const animatedScale = zoomSpring.current.value;
    let animatedCenterX = cursorSpringX.current.value;
    let animatedCenterY = cursorSpringY.current.value;

    if (captureMode === 'window') {
      animatedCenterX = Math.max(0.0, Math.min(1.0, animatedCenterX));
      animatedCenterY = Math.max(0.0, Math.min(1.0, animatedCenterY));
    }

    const meshCenterX = animatedCenterX - 0.5;
    const meshCenterY = -(animatedCenterY - 0.5);

    let offsetX = -meshCenterX * (animatedScale - 1) * planeWidth;
    let offsetY = -meshCenterY * (animatedScale - 1) * planeHeight;

    if (captureMode === 'window' && animatedScale > 1.0) {
      const anchorX = getWindowZoomAnchor(animatedCenterX, animatedScale);
      const anchorY = getWindowZoomAnchor(animatedCenterY, animatedScale);
      const sourceX = (animatedCenterX - 0.5) * planeWidth;
      const sourceY = (0.5 - animatedCenterY) * planeHeight;
      const anchorLocalX = (anchorX - 0.5) * planeWidth;
      const anchorLocalY = (0.5 - anchorY) * planeHeight;
      offsetX = anchorLocalX - sourceX * animatedScale;
      offsetY = anchorLocalY - sourceY * animatedScale;
    }

    const MIN_VELOCITY_THRESHOLD = 0.15;
    const panVelocity = Math.sqrt(
      cursorSpringX.current.velocity ** 2 +
      cursorSpringY.current.velocity ** 2
    );
    const velocityScale = Math.max(0.1, animatedScale);
    velocityRef.current = {
      scale: 0,
      x: panVelocity > MIN_VELOCITY_THRESHOLD ? cursorSpringX.current.velocity * velocityScale * 10 : 0,
      y: panVelocity > MIN_VELOCITY_THRESHOLD ? cursorSpringY.current.velocity * velocityScale * 10 : 0,
    };

    if (captureMode === 'window') {
      if (groupRef.current) {
        groupRef.current.scale.set(animatedScale, animatedScale, 1);
        groupRef.current.position.set(offsetX, offsetY, 0);
      }
      meshRef.current.scale.set(planeWidth, planeHeight, 1);
      meshRef.current.position.set(0, 0, 0);
    } else {
      if (groupRef.current) {
        groupRef.current.scale.set(1, 1, 1);
        groupRef.current.position.set(0, 0, 0);
      }
      meshRef.current.scale.set(
        planeWidth * animatedScale,
        planeHeight * animatedScale,
        1
      );
      meshRef.current.position.set(offsetX, offsetY, 0);
    }

    videoTransformRef.current.scale = animatedScale;
    videoTransformRef.current.offsetX = offsetX;
    videoTransformRef.current.offsetY = offsetY;
    videoTransformRef.current.viewportWidth = viewport.width;
    videoTransformRef.current.viewportHeight = viewport.height;
    videoTransformRef.current.planeWidth = planeWidth;
    videoTransformRef.current.planeHeight = planeHeight;
  });

  useEffect(() => {
    window.__TARANTINO_CURRENT_TIME = currentTime;
  }, [currentTime]);

  if (!videoFilePath) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
      </group>
    );
  }

  if (videoError) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
        <Text
          position={[0, 0, 0.1]}
          fontSize={0.3}
          color="#ff5555"
          anchorX="center"
          anchorY="middle"
        >
          {videoError}
        </Text>
      </group>
    );
  }

  if (!videoUrl) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
        <Text
          position={[0, 0, 0.1]}
          fontSize={0.4}
          color="#6272a4"
          anchorX="center"
          anchorY="middle"
        >
          Loading video...
        </Text>
      </group>
    );
  }

  return (
    <group ref={groupRef}>
      <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
      <VideoShadow width={planeWidth} height={planeHeight} settings={visualSettings} />
      <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
        <planeGeometry args={[1, 1]} />
        <Suspense fallback={<VideoFallback />}>
          <VideoMaterial
            videoUrl={videoUrl}
            isPlaying={isPlaying}
            cornerRadius={visualSettings.cornerRadius}
            aspectRatio={planeWidth / planeHeight}
            cleanupWindowCorners={captureMode === 'window'}
          />
        </Suspense>
      </mesh>
    </group>
  );
};
