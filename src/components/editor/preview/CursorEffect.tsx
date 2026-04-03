import React, { useMemo, useRef, useEffect, useState } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { Effect, BlendFunction } from 'postprocessing';
import * as THREE from 'three';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { cursorFragmentShader } from './shaders/cursor.glsl';

// --- Types ---

interface TrailPosition {
  x: number; // screen UV
  y: number;
}

// --- Helpers ---

function parseColor(hex: string): { r: number; g: number; b: number } {
  const match = hex.match(/^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i);
  if (match) {
    return {
      r: parseInt(match[1], 16) / 255,
      g: parseInt(match[2], 16) / 255,
      b: parseInt(match[3], 16) / 255,
    };
  }
  return { r: 0.39, g: 0.71, b: 1.0 };
}

// --- Effect implementation ---

class CursorEffectImpl extends Effect {
  constructor() {
    const trailDefault = new Array(30).fill(null).map(() => new THREE.Vector4(0, 0, 0, 0));

    super('CursorEffect', cursorFragmentShader, {
      blendFunction: BlendFunction.NORMAL,
      uniforms: new Map<string, THREE.Uniform>([
        ['uCursorX', new THREE.Uniform(0.0)],
        ['uCursorY', new THREE.Uniform(0.0)],
        ['uCursorScale', new THREE.Uniform(3.0)],
        ['uCursorOpacity', new THREE.Uniform(0.0)],
        ['uCursorRotation', new THREE.Uniform(0.0)],
        ['uCursorStyle', new THREE.Uniform(0.0)],
        ['uIsClicking', new THREE.Uniform(0.0)],
        ['uClickEffect', new THREE.Uniform(2.0)],
        ['uCursorColorR', new THREE.Uniform(1.0)],
        ['uCursorColorG', new THREE.Uniform(1.0)],
        ['uCursorColorB', new THREE.Uniform(1.0)],
        ['uHighlightColorR', new THREE.Uniform(1.0)],
        ['uHighlightColorG', new THREE.Uniform(0.42)],
        ['uHighlightColorB', new THREE.Uniform(0.42)],
        ['uRippleColorR', new THREE.Uniform(0.39)],
        ['uRippleColorG', new THREE.Uniform(0.71)],
        ['uRippleColorB', new THREE.Uniform(1.0)],
        ['uShadowIntensity', new THREE.Uniform(30.0)],
        ['uRippleProgress', new THREE.Uniform(0.0)],
        ['uRippleX', new THREE.Uniform(0.0)],
        ['uRippleY', new THREE.Uniform(0.0)],
        ['uCircleHlProgress', new THREE.Uniform(0.0)],
        ['uCircleHlX', new THREE.Uniform(0.0)],
        ['uCircleHlY', new THREE.Uniform(0.0)],
        ['uTrailEnabled', new THREE.Uniform(0.0)],
        ['uTrailCount', new THREE.Uniform(0.0)],
        ['uTrailOpacity', new THREE.Uniform(0.5)],
        ['uTrailPoints', new THREE.Uniform(trailDefault)],
        ['uResolutionX', new THREE.Uniform(1920.0)],
        ['uResolutionY', new THREE.Uniform(1080.0)],
        ['uVideoClipEnabled', new THREE.Uniform(0.0)],
        ['uVideoClipMinX', new THREE.Uniform(0.0)],
        ['uVideoClipMinY', new THREE.Uniform(0.0)],
        ['uVideoClipMaxX', new THREE.Uniform(99999.0)],
        ['uVideoClipMaxY', new THREE.Uniform(99999.0)],
      ]),
    });
  }
}

// --- React component ---

interface CursorEffectProps {
  sidecarPath: string;
  videoWidth: number;
  videoHeight: number;
  visible: boolean;
  videoTransformRef: React.MutableRefObject<{
    scale: number;
    offsetX: number;
    offsetY: number;
    viewportWidth: number;
    viewportHeight: number;
    planeWidth: number;
    planeHeight: number;
  }>;
}

export const CursorEffect: React.FC<CursorEffectProps> = ({
  sidecarPath,
  videoWidth,
  videoHeight,
  visible,
  videoTransformRef,
}) => {
  const effect = useMemo(() => new CursorEffectImpl(), []);
  const { size } = useThree();

  // Pre-computed trajectory from backend (same simulation as export)
  // Each frame: [x, y, opacity, isClicking, rippleProgress, rippleX, rippleY]
  const [trajectory, setTrajectory] = useState<number[][] | null>(null);
  const trailHistory = useRef<TrailPosition[]>([]);

  // Load pre-computed cursor trajectory from backend
  useEffect(() => {
    const load = async () => {
      if (!sidecarPath) return;

      try {
        const store = useEditorStore.getState();
        const duration = store.duration || 0;
        if (duration <= 0) return;

        const result = await invoke<string>('compute_cursor_trajectory', {
          mouseJsonPath: sidecarPath,
          durationMs: duration,
          fps: 60,
          videoWidth: videoWidth,
          videoHeight: videoHeight,
          cursorScale: store.visualSettings.cursorScale ?? 3.0,
        });

        const frames = JSON.parse(result) as number[][];
        console.log(`[CursorEffect] Loaded pre-computed trajectory: ${frames.length} frames`);
        setTrajectory(frames);
      } catch (error) {
        console.error('[CursorEffect] Failed to compute cursor trajectory:', error);
      }
    };

    load();
  }, [sidecarPath, videoWidth, videoHeight]);

  // Per-frame rendering using pre-computed trajectory
  useFrame(() => {
    if (!visible || !trajectory || trajectory.length === 0) {
      effect.uniforms.get('uCursorOpacity')!.value = 0;
      return;
    }

    const store = useEditorStore.getState();
    const vs = store.visualSettings;
    const duration = store.duration || 0;

    // Get current video time
    let currentTime = 0;
    const video = (window as any).__TARANTINO_VIDEO_ELEMENT;
    if (video && video.duration > 0) {
      currentTime = video.currentTime * 1000;
    } else {
      const editorTime = (window as any).__TARANTINO_CURRENT_TIME;
      if (editorTime !== undefined) currentTime = editorTime;
    }

    // Map time to frame index (same as export: frame_idx = time * fps / 1000)
    const frameIdx = Math.min(
      Math.floor((currentTime / 1000) * 60),
      trajectory.length - 1
    );
    const frame = trajectory[Math.max(0, frameIdx)];
    // frame: [x, y, opacity, isClicking, rippleProgress, rippleX, rippleY]
    const cursorX = frame[0];
    const cursorY = frame[1];
    const opacity = vs.hideCursor ? 0 : frame[2];
    const isClicking = frame[3] > 0.5;
    const rippleProgress = frame[4];
    const rippleNormX = frame[5];
    const rippleNormY = frame[6];

    const effectiveCursorStyle = vs.alwaysUsePointer ? 'pointer' : vs.cursorStyle;

    // --- Coordinate transform: video-normalized → screen UV ---
    // Three.js postprocessing UV is Y-up (OpenGL convention): (0,0) = bottom-left, (1,1) = top-right.
    // World Y-up: positive wy → higher screen position → higher v.
    const transform = videoTransformRef.current;
    const toScreenUV = (normX: number, normY: number) => {
      const lx = normX - 0.5;
      const ly = 0.5 - normY;
      const wx = lx * transform.planeWidth * transform.scale + transform.offsetX;
      const wy = ly * transform.planeHeight * transform.scale + transform.offsetY;
      return {
        u: 0.5 + wx / transform.viewportWidth,
        v: 0.5 + wy / transform.viewportHeight,
      };
    };

    const { u: screenU, v: screenV } = toScreenUV(cursorX, cursorY);

    // Pixel density: scale cursor so it matches export 1:1
    const canvasPixelHeight = size.height * (window.devicePixelRatio || 1);
    const videoPlaneCanvasPixels = (transform.planeHeight / transform.viewportHeight) * canvasPixelHeight;
    const densityScale = videoPlaneCanvasPixels / videoHeight;
    const adjustedCursorScale = vs.cursorScale * densityScale;

    // Trail
    if (vs.cursorTrailEnabled && opacity > 0.01) {
      trailHistory.current.push({ x: screenU, y: screenV });
      while (trailHistory.current.length > vs.cursorTrailLength) trailHistory.current.shift();
    } else {
      trailHistory.current = [];
    }

    // Ripple screen position
    const { u: rippleScreenX, v: rippleScreenY } = rippleProgress > 0.001
      ? toScreenUV(rippleNormX, rippleNormY)
      : { u: 0, v: 0 };

    // Style enum
    const styleMap: Record<string, number> = { pointer: 0, circle: 1, filled: 2, outline: 3, dotted: 4 };
    const styleVal = styleMap[effectiveCursorStyle] ?? 0;

    // Click effect enum
    const clickEffectMap: Record<string, number> = { none: 0, circle: 1, ripple: 2 };
    const clickEffectVal = clickEffectMap[vs.clickEffect] ?? 2;

    // Colors
    const cursorCol = parseColor(isClicking ? vs.cursorHighlightColor : vs.cursorColor);
    const hlCol = parseColor(vs.cursorHighlightColor);
    const rippleCol = parseColor(vs.cursorRippleColor);

    // Resolution
    const pixelRatio = window.devicePixelRatio || 1;
    const resX = size.width * pixelRatio;
    const resY = size.height * pixelRatio;

    // Set uniforms
    const u = effect.uniforms;
    u.get('uCursorX')!.value = screenU;
    u.get('uCursorY')!.value = screenV;
    u.get('uCursorScale')!.value = adjustedCursorScale;
    u.get('uCursorOpacity')!.value = opacity;
    u.get('uCursorRotation')!.value = 0;
    u.get('uCursorStyle')!.value = styleVal;
    u.get('uIsClicking')!.value = isClicking ? 1.0 : 0.0;
    u.get('uClickEffect')!.value = clickEffectVal;
    u.get('uCursorColorR')!.value = cursorCol.r;
    u.get('uCursorColorG')!.value = cursorCol.g;
    u.get('uCursorColorB')!.value = cursorCol.b;
    u.get('uHighlightColorR')!.value = hlCol.r;
    u.get('uHighlightColorG')!.value = hlCol.g;
    u.get('uHighlightColorB')!.value = hlCol.b;
    u.get('uRippleColorR')!.value = rippleCol.r;
    u.get('uRippleColorG')!.value = rippleCol.g;
    u.get('uRippleColorB')!.value = rippleCol.b;
    u.get('uShadowIntensity')!.value = vs.cursorShadowIntensity;
    // Route animation data to the correct effect uniforms
    if (clickEffectVal === 1) {
      // Circle: use ripple animation data for circle highlight
      u.get('uRippleProgress')!.value = 0;
      u.get('uRippleX')!.value = 0;
      u.get('uRippleY')!.value = 0;
      u.get('uCircleHlProgress')!.value = rippleProgress;
      u.get('uCircleHlX')!.value = rippleScreenX;
      u.get('uCircleHlY')!.value = rippleScreenY;
    } else {
      // Ripple or None
      u.get('uRippleProgress')!.value = rippleProgress;
      u.get('uRippleX')!.value = rippleScreenX;
      u.get('uRippleY')!.value = rippleScreenY;
      u.get('uCircleHlProgress')!.value = 0;
      u.get('uCircleHlX')!.value = 0;
      u.get('uCircleHlY')!.value = 0;
    }
    u.get('uTrailEnabled')!.value = vs.cursorTrailEnabled ? 1.0 : 0.0;
    u.get('uTrailCount')!.value = trailHistory.current.length;
    u.get('uTrailOpacity')!.value = vs.cursorTrailOpacity;
    u.get('uResolutionX')!.value = resX;
    u.get('uResolutionY')!.value = resY;

    // Video plane bounds clipping (window mode only — display mode is unaffected)
    if (store.captureMode === 'window') {
      const halfW = transform.planeWidth * transform.scale / 2;
      const halfH = transform.planeHeight * transform.scale / 2;
      // Convert world coords to pixel coords (matching shader pixel space)
      u.get('uVideoClipEnabled')!.value = 1.0;
      u.get('uVideoClipMinX')!.value = (0.5 + (transform.offsetX - halfW) / transform.viewportWidth) * resX;
      u.get('uVideoClipMinY')!.value = (0.5 + (transform.offsetY - halfH) / transform.viewportHeight) * resY;
      u.get('uVideoClipMaxX')!.value = (0.5 + (transform.offsetX + halfW) / transform.viewportWidth) * resX;
      u.get('uVideoClipMaxY')!.value = (0.5 + (transform.offsetY + halfH) / transform.viewportHeight) * resY;
    } else {
      u.get('uVideoClipEnabled')!.value = 0.0;
    }

    // Trail points
    const trailUniform = u.get('uTrailPoints')!.value as THREE.Vector4[];
    for (let i = 0; i < 30; i++) {
      if (i < trailHistory.current.length) {
        const tp = trailHistory.current[i];
        const progress = i / trailHistory.current.length;
        trailUniform[i].set(tp.x, tp.y, progress * vs.cursorTrailOpacity * opacity, (2 + progress * 4) * adjustedCursorScale);
      } else {
        trailUniform[i].set(0, 0, 0, 0);
      }
    }
  });

  return <primitive object={effect} />;
};

CursorEffect.displayName = 'CursorEffect';
