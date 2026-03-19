import { useCallback } from 'react';
import type { SpringState, SpringConfig } from '../types';

/**
 * Spring physics step function
 * Simulates a damped spring for smooth cursor movement
 */
export const springStep = (
  current: number,
  target: number,
  velocity: number,
  config: SpringConfig,
  dt: number
): SpringState => {
  const { tension, friction, mass } = config;
  const safeDt = Math.min(dt, 0.064); // Cap at ~15fps minimum

  const displacement = current - target;
  const springForce = -tension * displacement;
  const dampingForce = -friction * velocity;
  const acceleration = (springForce + dampingForce) / mass;

  const newVelocity = velocity + acceleration * safeDt;
  const newValue = current + newVelocity * safeDt;

  // Snap to target if close enough and velocity is low
  if (Math.abs(displacement) < 0.5 && Math.abs(newVelocity) < 0.5) {
    return { value: target, velocity: 0 };
  }

  return { value: newValue, velocity: newVelocity };
};

/**
 * Linear interpolation helper
 */
export const lerp = (a: number, b: number, t: number): number => {
  return a + (b - a) * Math.min(1, Math.max(0, t));
};

/**
 * Parse hex color to RGB values
 */
export const parseColor = (hex: string): { r: number; g: number; b: number } => {
  const match = hex.match(/^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i);
  if (match) {
    return {
      r: parseInt(match[1], 16),
      g: parseInt(match[2], 16),
      b: parseInt(match[3], 16)
    };
  }
  return { r: 100, g: 180, b: 255 }; // default blue
};

/**
 * Hook for filtering mouse shake events
 */
export const useShakeFilter = (removeCursorShakes: boolean) => {
  return useCallback((events: any[], threshold: number): any[] => {
    if (!removeCursorShakes || threshold <= 0) return events;

    return events.filter((event, i) => {
      if (i === 0 || event.event_type.Move === undefined) return true;

      // Find previous move event
      let prevMoveIndex = i - 1;
      while (prevMoveIndex >= 0 && events[prevMoveIndex].event_type.Move === undefined) {
        prevMoveIndex--;
      }
      if (prevMoveIndex < 0) return true;

      const prev = events[prevMoveIndex];
      const dx = event.x - prev.x;
      const dy = event.y - prev.y;
      const distance = Math.sqrt(dx * dx + dy * dy);
      const timeDelta = event.timestamp - prev.timestamp;

      // Remove if small movement in short time (shake)
      return !(distance < threshold && timeDelta < 100);
    });
  }, [removeCursorShakes]);
};

/**
 * Transform normalized coordinates (0-1) to canvas coordinates
 * Syncs cursor position with R3F video plane transform
 */
export const transformToCanvas = (
  normX: number,
  normY: number,
  canvasWidth: number,
  canvasHeight: number,
  videoTransform: {
    scale?: number;
    offsetX?: number;
    offsetY?: number;
    viewportWidth?: number;
    viewportHeight?: number;
    planeWidth?: number;
    planeHeight?: number;
  } = {}
): { x: number; y: number } => {
  const {
    scale: zoomScale = 1,
    offsetX: videoOffsetX = 0,
    offsetY: videoOffsetY = 0,
    viewportWidth = 1,
    viewportHeight = 1,
    planeWidth = 1,
    planeHeight = 1
  } = videoTransform;

  // Calculate pixels per R3F unit (viewport fills canvas)
  const pixelsPerUnitX = canvasWidth / viewportWidth;
  const pixelsPerUnitY = canvasHeight / viewportHeight;

  // Map normalized video coords to plane local coords (before scale/offset)
  // The plane geometry is 1x1 centered at origin, scaled by planeWidth/planeHeight
  // normX: 0=left, 1=right -> localX: -0.5 to +0.5
  // normY: 0=top, 1=bottom (video coords) -> localY: +0.5 to -0.5 (R3F Y is up)
  const localX = normX - 0.5;
  const localY = 0.5 - normY; // Invert Y for R3F coordinate system

  // Apply the same transform as the R3F video mesh:
  // mesh.scale = [planeWidth * zoomScale, planeHeight * zoomScale, 1]
  // mesh.position = [offsetX, offsetY, 0]
  const worldX = localX * planeWidth * zoomScale + videoOffsetX;
  const worldY = localY * planeHeight * zoomScale + videoOffsetY;

  // Convert R3F world coordinates to canvas pixels
  // R3F: origin at center, Y up
  // Canvas: origin at top-left, Y down
  return {
    x: (canvasWidth / 2) + worldX * pixelsPerUnitX,
    y: (canvasHeight / 2) - worldY * pixelsPerUnitY
  };
};
