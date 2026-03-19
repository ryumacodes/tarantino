import { useRef } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { useEditorStore, SPRING_PRESETS } from '../../../../stores/editor';

// Spring physics configuration type
interface SpringConfig {
  tension: number;
  friction: number;
  mass: number;
}

// Spring state for a single value
interface SpringState {
  value: number;
  velocity: number;
}

// Spring physics step function (Screen Studio style)
// Uses Hooke's law with damping: F = -kx - cv
const springStep = (
  current: number,
  target: number,
  velocity: number,
  config: SpringConfig,
  dt: number // delta time in seconds
): SpringState => {
  const { tension, friction, mass } = config;

  // Cap delta to prevent instability during frame drops
  const safeDt = Math.min(dt, 0.064); // Max ~16fps worth of time

  // Spring force: F = -k * displacement (Hooke's law)
  const displacement = current - target;
  const springForce = -tension * displacement;

  // Damping force: F = -c * velocity
  const dampingForce = -friction * velocity;

  // Total force and acceleration: a = F / m
  const acceleration = (springForce + dampingForce) / mass;

  // Update velocity and position using semi-implicit Euler
  const newVelocity = velocity + acceleration * safeDt;
  const newValue = current + newVelocity * safeDt;

  // Snap to target if close enough (prevents micro-oscillations)
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

interface UseSpringZoomOptions {
  meshRef: React.RefObject<THREE.Mesh>;
  planeWidth: number;
  planeHeight: number;
  velocityRef: React.MutableRefObject<{ scale: number; x: number; y: number }>;
  videoTransformRef: React.MutableRefObject<VideoTransform>;
}

export function useSpringZoom({
  meshRef,
  planeWidth,
  planeHeight,
  velocityRef,
  videoTransformRef,
}: UseSpringZoomOptions) {
  const { viewport } = useThree();
  const { zoomAnalysis, currentTime, getCursorAtTime, visualSettings } = useEditorStore();

  // Spring state refs for smooth animations (Screen Studio style)
  const zoomSpring = useRef<SpringState>({ value: 1, velocity: 0 });
  const cursorSpringX = useRef<SpringState>({ value: 0.5, velocity: 0 });
  const cursorSpringY = useRef<SpringState>({ value: 0.5, velocity: 0 });

  // Get spring configs from presets
  const zoomConfig = SPRING_PRESETS[visualSettings.zoomSpeedPreset];
  const cursorConfig = SPRING_PRESETS[visualSettings.cursorSpeedPreset];

  // Zoom pan spring: responsive but smooth cursor following during zoom
  const zoomPanConfig: SpringConfig = {
    tension: 160,
    friction: 32,
    mass: 1.0
  };

  useFrame((state, delta) => {
    if (!meshRef.current) return;

    let targetScale = 1;
    let targetCenterX = 0.5;
    let targetCenterY = 0.5;
    let isZooming = false;

    // Phase transition duration (ms) for smooth blending between zoom phases
    const transitionDuration = 400;

    // How close (ms) a block must be to keep the zoom engaged
    const proximityThreshold = 600;

    if (zoomAnalysis && zoomAnalysis.zoom_blocks.length > 0) {
      const blocks = zoomAnalysis.zoom_blocks;

      // Find the active block (currentTime inside its range)
      const activeBlock = blocks.find(
        b => currentTime >= b.start_time && currentTime <= b.end_time
      );

      if (activeBlock) {
        isZooming = true;
        targetScale = activeBlock.zoom_factor;

        const timeInBlock = currentTime - activeBlock.start_time;
        const timeUntilEnd = activeBlock.end_time - currentTime;

        // Determine the current target center from the block's centers array.
        // Pan to each center as its timestamp arrives (merged clicks re-center).
        const centers = activeBlock.centers ?? [];
        let activeCenterX = activeBlock.center_x;
        let activeCenterY = activeBlock.center_y;

        if (centers.length > 0) {
          // Find the most recent center whose time has passed
          for (const c of centers) {
            if (currentTime >= c.time) {
              activeCenterX = c.x;
              activeCenterY = c.y;
            }
          }
        }

        // Also blend with cursor position during the follow phase
        const cursorPos = getCursorAtTime(currentTime);
        const cursorX = cursorPos?.x ?? activeCenterX;
        const cursorY = cursorPos?.y ?? activeCenterY;

        // Zoom-in/out phase blending
        const zoomInWeight = Math.max(0, 1 - timeInBlock / transitionDuration);
        const zoomOutWeight = Math.max(0, 1 - timeUntilEnd / transitionDuration);
        const followWeight = Math.max(0, 1 - zoomInWeight - zoomOutWeight);

        targetCenterX = activeCenterX * zoomInWeight + cursorX * followWeight + 0.5 * zoomOutWeight;
        targetCenterY = activeCenterY * zoomInWeight + cursorY * followWeight + 0.5 * zoomOutWeight;
      }
    }

    if (!isZooming) {
      targetCenterX = 0.5;
      targetCenterY = 0.5;
    }

    // Viewport edge clamping: prevent showing outside video bounds
    const currentScale = zoomSpring.current.value;
    if (currentScale > 1.0) {
      const halfVisible = 0.5 / currentScale;
      targetCenterX = Math.max(halfVisible, Math.min(1 - halfVisible, targetCenterX));
      targetCenterY = Math.max(halfVisible, Math.min(1 - halfVisible, targetCenterY));
    }

    targetCenterX = Math.max(0.0, Math.min(1.0, targetCenterX));
    targetCenterY = Math.max(0.0, Math.min(1.0, targetCenterY));

    const panSpringConfig = isZooming ? zoomPanConfig : cursorConfig;
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
      zoomConfig,
      delta
    );

    let animatedCenterX = cursorSpringX.current.value;
    let animatedCenterY = cursorSpringY.current.value;
    const animatedScale = zoomSpring.current.value;

    // Edge clamp the animated values too (spring may overshoot slightly)
    if (animatedScale > 1.0) {
      const halfVisible = 0.5 / animatedScale;
      animatedCenterX = Math.max(halfVisible, Math.min(1 - halfVisible, animatedCenterX));
      animatedCenterY = Math.max(halfVisible, Math.min(1 - halfVisible, animatedCenterY));
    }

    const meshCenterX = animatedCenterX - 0.5;
    const meshCenterY = -(animatedCenterY - 0.5);

    const offsetX = -meshCenterX * (animatedScale - 1) * planeWidth;
    const offsetY = -meshCenterY * (animatedScale - 1) * planeHeight;

    const MIN_VELOCITY_THRESHOLD = 0.15;
    const panVelocity = Math.sqrt(
      cursorSpringX.current.velocity ** 2 +
      cursorSpringY.current.velocity ** 2
    );
    velocityRef.current = {
      scale: Math.abs(zoomSpring.current.velocity) > 0.01 ? zoomSpring.current.velocity : 0,
      x: panVelocity > MIN_VELOCITY_THRESHOLD ? cursorSpringX.current.velocity : 0,
      y: panVelocity > MIN_VELOCITY_THRESHOLD ? cursorSpringY.current.velocity : 0,
    };

    meshRef.current.scale.set(
      planeWidth * animatedScale,
      planeHeight * animatedScale,
      1
    );
    meshRef.current.position.set(offsetX, offsetY, 0);

    videoTransformRef.current.scale = animatedScale;
    videoTransformRef.current.offsetX = offsetX;
    videoTransformRef.current.offsetY = offsetY;
    videoTransformRef.current.viewportWidth = viewport.width;
    videoTransformRef.current.viewportHeight = viewport.height;
    videoTransformRef.current.planeWidth = planeWidth;
    videoTransformRef.current.planeHeight = planeHeight;
  });
}
