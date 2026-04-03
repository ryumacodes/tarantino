import React, { useRef, useState, useEffect, Suspense } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import { Text } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore, SPRING_PRESETS } from '../../../stores/editor';
import { VideoMaterial, VideoFallback } from './VideoMaterial';
import { BackgroundPlane } from './BackgroundPlane';
import { VideoShadow } from './VideoShadow';

// Spring physics types
interface SpringConfig {
  tension: number;
  friction: number;
  mass: number;
}

interface SpringState {
  value: number;
  velocity: number;
}

// Spring physics step function
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

export const VideoViewer: React.FC<VideoViewerProps> = ({
  showMouseOverlay,
  isPlaying,
  velocityRef,
  videoTransformRef,
  previewZoom = 1
}) => {
  const { videoFilePath, zoomAnalysis, currentTime, loadMouseEvents, visualSettings, displayResolution, captureMode } = useEditorStore();
  const meshRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);
  const { viewport } = useThree();

  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoError, setVideoError] = useState<string | null>(null);

  useEffect(() => {
    const loadVideo = async () => {
      if (videoFilePath) {
        try {
          console.log('VideoViewer: Converting file path to asset URL:', videoFilePath);
          const url = convertFileSrc(videoFilePath);
          console.log('VideoViewer: Asset URL:', url);
          setVideoUrl(url);
          setVideoError(null);

          const mouseFilePath = videoFilePath.replace('.mp4', '.mouse.json');
          console.log('VideoViewer: Loading mouse events from:', mouseFilePath);
          loadMouseEvents(mouseFilePath);
        } catch (err) {
          console.error('VideoViewer: Failed to convert file path:', err);
          setVideoError(`Failed to load video: ${err}`);
        }
      }
    };
    loadVideo();
  }, [videoFilePath, loadMouseEvents]);

  // Calculate plane dimensions
  // For window recordings, use the export canvas aspect ratio (e.g. 16:9)
  // For display recordings, use the display's native aspect ratio
  const ASPECT_MAP: Record<string, number> = {
    '16:9': 16/9, '9:16': 9/16, '4:3': 4/3, '1:1': 1, '21:9': 21/9,
  };
  const videoAspect = captureMode === 'window'
    ? (ASPECT_MAP[visualSettings.aspectRatio]
        // 'auto' → use source video aspect (matches getExportDimensions behaviour)
        || (displayResolution ? displayResolution.width / displayResolution.height : 16 / 9))
    : displayResolution
      ? displayResolution.width / displayResolution.height
      : 16 / 9;
  const viewportAspect = viewport.width / viewport.height;

  // Base canvas dimensions (used for BackgroundPlane in all modes)
  // Fit within viewport (use min of both axes to prevent overflow)
  let basePlaneWidth: number, basePlaneHeight: number;
  const fitW = viewport.width * previewZoom;
  const fitH = viewport.height * previewZoom;
  if (videoAspect > viewportAspect) {
    // Video is wider than viewport — constrain by width
    basePlaneWidth = fitW;
    basePlaneHeight = fitW / videoAspect;
  } else {
    // Video is taller than viewport — constrain by height
    basePlaneHeight = fitH;
    basePlaneWidth = fitH * videoAspect;
  }
  // Clamp to never exceed viewport in either dimension
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
    // Window mode: canvas is the export ratio (e.g. 16:9).
    // Video is aspect-fit inside — background fills the rest naturally.
    // Padding matches export: padding % is removed from each side.
    const sourceAspect = displayResolution
      ? displayResolution.width / displayResolution.height
      : videoAspect;
    const inset = Math.max(0.01, 1 - 2 * (visualSettings.padding / 100));
    if (sourceAspect > videoAspect) {
      planeWidth = basePlaneWidth * inset;
      planeHeight = (basePlaneWidth / sourceAspect) * inset;
    } else {
      planeHeight = basePlaneHeight * inset;
      planeWidth = (basePlaneHeight * sourceAspect) * inset;
    }
  } else {
    // Display mode: padding matches export — padding % is removed from each side
    const paddingFactor = 1 - 2 * (visualSettings.padding / 100);
    planeWidth = basePlaneWidth * Math.max(0.01, paddingFactor);
    planeHeight = basePlaneHeight * Math.max(0.01, paddingFactor);
  }

  const { getCursorAtTime } = useEditorStore();

  // Spring state refs
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

  const getZoomPhaseDuration = (config: SpringConfig): number => {
    if (config.tension <= 170) return 450;
    if (config.tension <= 210) return 350;
    if (config.tension <= 280) return 250;
    return 150;
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

        // Resolve per-block spring configs
        const blockInConfig = activeBlock.zoom_in_speed
          ? SPRING_PRESETS[activeBlock.zoom_in_speed] : globalZoomConfig;
        const blockOutConfig = activeBlock.zoom_out_speed
          ? SPRING_PRESETS[activeBlock.zoom_out_speed] : globalZoomConfig;

        // Compute per-block phase durations
        const inPhaseDuration = getZoomPhaseDuration(blockInConfig);
        const outPhaseDuration = getZoomPhaseDuration(blockOutConfig);

        // Snap center springs only when entering zoom from unzoomed state
        const blockKey = `${activeBlock.start_time}-${activeBlock.end_time}`;
        if (prevActiveBlockRef.current !== blockKey) {
          prevActiveBlockRef.current = blockKey;
          const alreadyZoomed = zoomSpring.current.value > 1.1;
          if (!alreadyZoomed) {
            cursorSpringX.current = { value: activeBlock.center_x, velocity: 0 };
            cursorSpringY.current = { value: activeBlock.center_y, velocity: 0 };
          }
        }

        const timeInBlock = currentTime - activeBlock.start_time;
        const timeUntilEnd = activeBlock.end_time - currentTime;

        if (timeInBlock < inPhaseDuration) {
          // Zoom-in phase
          zoomSpringConfig = blockInConfig;
          targetCenterX = activeBlock.center_x;
          targetCenterY = activeBlock.center_y;
        } else if (timeUntilEnd > outPhaseDuration) {
          // Follow phase
          isFollowPhase = true;
          zoomSpringConfig = blockInConfig;
          const cursorPos = getCursorAtTime(currentTime);
          if (cursorPos) {
            targetCenterX = cursorPos.x;
            targetCenterY = cursorPos.y;
          } else {
            targetCenterX = activeBlock.center_x;
            targetCenterY = activeBlock.center_y;
          }
        } else {
          // Zoom-out phase
          zoomSpringConfig = blockOutConfig;
          targetCenterX = cursorSpringX.current.value;
          targetCenterY = cursorSpringY.current.value;
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

    // Follow phase uses cursorConfig for responsive tracking; other zoom phases use sluggish zoomPanConfig
    const panSpringConfig = isFollowPhase ? cursorConfig : (isZooming ? zoomPanConfig : cursorConfig);
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

    const animatedCenterX = cursorSpringX.current.value;
    const animatedCenterY = cursorSpringY.current.value;
    const animatedScale = zoomSpring.current.value;

    const meshCenterX = animatedCenterX - 0.5;
    const meshCenterY = -(animatedCenterY - 0.5);

    const offsetX = -meshCenterX * (animatedScale - 1) * planeWidth;
    const offsetY = -meshCenterY * (animatedScale - 1) * planeHeight;

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
      // Window mode: zoom the entire canvas (video + background + shadow) as one unit
      // so background bars zoom with the video, matching display/screen behavior.
      // Only groupRef is touched here — display mode path below is completely unchanged.
      if (groupRef.current) {
        groupRef.current.scale.set(animatedScale, animatedScale, 1);
        groupRef.current.position.set(offsetX, offsetY, 0);
      }
      meshRef.current.scale.set(planeWidth, planeHeight, 1);
      meshRef.current.position.set(0, 0, 0);
    } else {
      // Display mode: unchanged — zoom just the video mesh
      // Reset group transform in case user switched from a window recording
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
    (window as any).__TARANTINO_CURRENT_TIME = currentTime;
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
            aspectRatio={videoAspect}
          />
        </Suspense>
      </mesh>
    </group>
  );
};
