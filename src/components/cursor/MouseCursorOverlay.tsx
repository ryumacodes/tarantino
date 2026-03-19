import React, { useRef, useEffect, useState, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore, SPRING_PRESETS } from '../../stores/editor';
import type {
  MouseEvent as CursorMouseEvent,
  Ripple,
  CircleHighlight,
  TrailPosition,
  CursorSpringState,
  MouseCursorOverlayProps
} from './types';
import { springStep, lerp, transformToCanvas, useShakeFilter } from './hooks/useCursorPhysics';
import { drawCursor, drawRipple, drawCircleHighlight, drawTrail } from './CursorRenderer';

const MouseCursorOverlay: React.FC<MouseCursorOverlayProps> = ({
  videoRef,
  sidecarPath,
  videoWidth = 1920,
  videoHeight = 1080,
  visible = true,
  videoDuration = 0,
  videoTransform,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [mouseEvents, setMouseEvents] = useState<CursorMouseEvent[]>([]);
  const [canvasSize, setCanvasSize] = useState({ width: 800, height: 600 });

  // Get visual settings from store
  const visualSettings = useEditorStore((state) => state.visualSettings);
  const duration = useEditorStore((state) => state.duration) || videoDuration;

  // Resize canvas to match container (for crisp rendering)
  useEffect(() => {
    if (!containerRef.current) return;

    const updateSize = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        setCanvasSize({
          width: Math.round(rect.width * dpr),
          height: Math.round(rect.height * dpr)
        });
      }
    };

    updateSize();
    const resizeObserver = new ResizeObserver(updateSize);
    resizeObserver.observe(containerRef.current);
    return () => resizeObserver.disconnect();
  }, []);

  const {
    cursorScale,
    cursorSmoothing,
    hideCursorWhenIdle,
    idleTimeout,
    cursorSpeedPreset,
    cursorColor,
    cursorHighlightColor,
    cursorRippleColor,
    cursorShadowIntensity,
    cursorTrailEnabled,
    cursorTrailLength,
    cursorTrailOpacity,
    cursorStyle,
    hideCursor,
    alwaysUsePointer,
    loopCursorPosition,
    clickEffect,
    cursorRotation,
    rotateCursorWhileMoving,
    rotationIntensity,
    stopCursorAtEnd,
    stopCursorDuration,
    removeCursorShakes,
    shakesThreshold,
  } = visualSettings;

  // Determine effective cursor style
  const effectiveCursorStyle = alwaysUsePointer ? 'pointer' : cursorStyle;

  // Animation state refs
  const cursorSpring = useRef<CursorSpringState>({ x: { value: 0, velocity: 0 }, y: { value: 0, velocity: 0 } });
  const targetCursor = useRef<{ x: number; y: number; isClicking: boolean } | null>(null);
  const lastMoveTime = useRef(0);
  const cursorOpacity = useRef(1);
  const ripples = useRef<Ripple[]>([]);
  const circleHighlights = useRef<CircleHighlight[]>([]);
  const rippleIdCounter = useRef(0);
  const lastFrameTime = useRef(performance.now());
  const lastClickState = useRef(false);
  const trailHistory = useRef<TrailPosition[]>([]);
  const horizontalVelocity = useRef(0);
  const lastXPosition = useRef(0);
  const firstMoveEvent = useRef<CursorMouseEvent | null>(null);
  const frozenCursorPosition = useRef<{ x: number; y: number } | null>(null);
  const hasFrozenCursor = useRef(false);

  // Use shake filter hook
  const filterShakes = useShakeFilter(removeCursorShakes);

  // Get mouse events from store as fallback
  const storeMouseEvents = useEditorStore((state) => state.mouseEvents);

  // Convert store event format to local MouseEvent format
  const convertStoreEvents = useCallback((events: typeof storeMouseEvents): CursorMouseEvent[] => {
    if (!events) return [];
    return events.map(e => {
      const eventType: CursorMouseEvent['event_type'] = {};
      if (e.base.event_type === 'Move') {
        eventType.Move = undefined;
      } else if (e.base.event_type.startsWith('ButtonPress')) {
        eventType.ButtonPress = { button: 'Left' };
      } else if (e.base.event_type.startsWith('ButtonRelease')) {
        eventType.ButtonRelease = { button: 'Left' };
      }
      return {
        timestamp: e.base.timestamp,
        x: e.base.x,
        y: e.base.y,
        event_type: eventType,
        display_id: e.base.display_id || undefined,
      };
    });
  }, []);

  // Load mouse events from sidecar file or use store fallback
  useEffect(() => {
    const loadMouseEventsFromSidecar = async () => {
      if (!sidecarPath) {
        if (storeMouseEvents && storeMouseEvents.length > 0) {
          const converted = convertStoreEvents(storeMouseEvents);
          const filteredEvents = filterShakes(converted, shakesThreshold);
          setMouseEvents(filteredEvents);
          const firstMove = filteredEvents.find((e: CursorMouseEvent) => 'Move' in e.event_type);
          firstMoveEvent.current = firstMove || null;
        }
        return;
      }

      try {
        const sidecarContent = await invoke<string>('read_sidecar_file', { path: sidecarPath });
        const sidecarData = JSON.parse(sidecarContent);

        let rawEvents: any[];
        let displayWidth = videoWidth;
        let displayHeight = videoHeight;
        let recordingArea: { x: number; y: number; width: number; height: number } | null = null;

        if (Array.isArray(sidecarData)) {
          rawEvents = sidecarData;
        } else if (sidecarData.mouse_events) {
          rawEvents = sidecarData.mouse_events;
          displayWidth = sidecarData.display_width || videoWidth;
          displayHeight = sidecarData.display_height || videoHeight;
          recordingArea = sidecarData.recording_area || null;
        } else {
          throw new Error('Invalid sidecar format');
        }

        const effectiveX = recordingArea?.x ?? 0;
        const effectiveY = recordingArea?.y ?? 0;
        const effectiveWidth = recordingArea?.width ?? displayWidth;
        const effectiveHeight = recordingArea?.height ?? displayHeight;

        console.log('[MouseCursorOverlay] Coordinate normalization:', {
          displayWidth,
          displayHeight,
          scaleFactor: sidecarData.scale_factor,
          recordingArea,
          effectiveX,
          effectiveY,
          effectiveWidth,
          effectiveHeight,
          sampleEvent: rawEvents[0] ? { x: rawEvents[0].base?.x || rawEvents[0].x, y: rawEvents[0].base?.y || rawEvents[0].y } : null
        });

        const convertedEvents: CursorMouseEvent[] = rawEvents.map((e: any) => {
          const event = e.base || e;
          let eventType: CursorMouseEvent['event_type'] = {};

          if (typeof event.event_type === 'string') {
            if (event.event_type === 'Move') eventType.Move = undefined;
            else if (event.event_type === 'ButtonPress') eventType.ButtonPress = { button: 'Left' };
            else if (event.event_type === 'ButtonRelease') eventType.ButtonRelease = { button: 'Left' };
          } else if (typeof event.event_type === 'object' && event.event_type !== null) {
            if ('Move' in event.event_type) eventType.Move = undefined;
            if ('ButtonPress' in event.event_type) eventType.ButtonPress = event.event_type.ButtonPress || { button: 'Left' };
            if ('ButtonRelease' in event.event_type) eventType.ButtonRelease = event.event_type.ButtonRelease || { button: 'Left' };
            if ('Wheel' in event.event_type) eventType.Wheel = event.event_type.Wheel;
          }

          const adjustedX = event.x - effectiveX;
          const adjustedY = event.y - effectiveY;
          const normalizedX = Math.max(0, Math.min(1, adjustedX / effectiveWidth));
          const normalizedY = Math.max(0, Math.min(1, adjustedY / effectiveHeight));

          return {
            timestamp: event.timestamp,
            x: normalizedX,
            y: normalizedY,
            event_type: eventType,
            display_id: event.display_id,
          };
        });

        const filteredEvents = filterShakes(convertedEvents, shakesThreshold);
        setMouseEvents(filteredEvents);
        const firstMove = filteredEvents.find((e: CursorMouseEvent) => 'Move' in e.event_type);
        firstMoveEvent.current = firstMove || null;
      } catch (error) {
        console.error('[MouseCursorOverlay] Failed to load mouse events:', error);
        if (storeMouseEvents && storeMouseEvents.length > 0) {
          const converted = convertStoreEvents(storeMouseEvents);
          const filteredEvents = filterShakes(converted, shakesThreshold);
          setMouseEvents(filteredEvents);
        }
      }
    };

    loadMouseEventsFromSidecar();
  }, [sidecarPath, filterShakes, shakesThreshold, storeMouseEvents, convertStoreEvents, videoWidth, videoHeight]);

  // Spawn effects
  const spawnRipple = useCallback((x: number, y: number) => {
    if (clickEffect !== 'ripple') return;
    ripples.current.push({
      id: rippleIdCounter.current++,
      x, y,
      startTime: performance.now(),
      duration: 600,
    });
    if (ripples.current.length > 10) ripples.current = ripples.current.slice(-10);
  }, [clickEffect]);

  const spawnCircleHighlight = useCallback((x: number, y: number) => {
    if (clickEffect !== 'circle') return;
    circleHighlights.current.push({
      id: rippleIdCounter.current++,
      x, y,
      startTime: performance.now(),
      duration: 300,
    });
    if (circleHighlights.current.length > 10) circleHighlights.current = circleHighlights.current.slice(-10);
  }, [clickEffect]);

  // Main animation loop
  useEffect(() => {
    if (!canvasRef.current || !visible) return;

    const video = videoRef.current || (window as any).__TARANTINO_VIDEO_ELEMENT;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const springConfig = SPRING_PRESETS[cursorSpeedPreset];
    let animationFrame: number;

    const animate = () => {
      const now = performance.now();
      const dt = (now - lastFrameTime.current) / 1000;
      lastFrameTime.current = now;

      let currentTime = 0;
      if (video && video.duration > 0) {
        currentTime = video.currentTime * 1000;
      } else {
        const editorTime = (window as any).__TARANTINO_CURRENT_TIME;
        if (editorTime !== undefined) currentTime = editorTime;
      }

      // Stop cursor at end
      const timeBeforeEnd = duration - currentTime;
      if (stopCursorAtEnd && stopCursorDuration > 0 && timeBeforeEnd < stopCursorDuration && timeBeforeEnd > 0) {
        if (!hasFrozenCursor.current && targetCursor.current) {
          frozenCursorPosition.current = { x: cursorSpring.current.x.value, y: cursorSpring.current.y.value };
          hasFrozenCursor.current = true;
        }
      } else {
        hasFrozenCursor.current = false;
        frozenCursorPosition.current = null;
      }

      // Find closest move event
      let closestEvent: CursorMouseEvent | null = null;
      let minTimeDiff = Infinity;

      for (const event of mouseEvents) {
        if ('Move' in event.event_type) {
          const timeDiff = Math.abs(event.timestamp - currentTime);
          if (timeDiff < minTimeDiff) {
            minTimeDiff = timeDiff;
            closestEvent = event;
          }
        }
      }

      // Check for clicks
      const clickEvents = mouseEvents.filter(event =>
        Math.abs(event.timestamp - currentTime) < 100 &&
        (event.event_type.ButtonPress || event.event_type.ButtonRelease)
      );
      const isClicking = clickEvents.some(event => event.event_type.ButtonPress);

      if (isClicking && !lastClickState.current && closestEvent) {
        spawnRipple(closestEvent.x, closestEvent.y);
        spawnCircleHighlight(closestEvent.x, closestEvent.y);
      }
      lastClickState.current = isClicking;

      // Calculate target position
      let targetX = closestEvent?.x || 0;
      let targetY = closestEvent?.y || 0;

      // Loop cursor position
      if (loopCursorPosition && firstMoveEvent.current && duration > 0) {
        const loopDuration = 500;
        if (timeBeforeEnd < loopDuration && timeBeforeEnd > 0) {
          const progress = 1 - (timeBeforeEnd / loopDuration);
          targetX = lerp(targetX, firstMoveEvent.current.x, progress);
          targetY = lerp(targetY, firstMoveEvent.current.y, progress);
        }
      }

      // Update target
      if (!hasFrozenCursor.current) {
        if (closestEvent && minTimeDiff < 500) {
          const newTarget = { x: targetX, y: targetY, isClicking };
          if (!targetCursor.current || targetCursor.current.x !== newTarget.x || targetCursor.current.y !== newTarget.y) {
            lastMoveTime.current = now;
          }
          targetCursor.current = newTarget;
        } else {
          targetCursor.current = null;
        }
      }

      // Spring physics
      if (targetCursor.current) {
        cursorSpring.current.x = springStep(cursorSpring.current.x.value, targetCursor.current.x, cursorSpring.current.x.velocity, springConfig, dt);
        cursorSpring.current.y = springStep(cursorSpring.current.y.value, targetCursor.current.y, cursorSpring.current.y.velocity, springConfig, dt);
      }

      // Idle fade
      if (hideCursorWhenIdle) {
        const timeSinceMove = now - lastMoveTime.current;
        if (timeSinceMove > idleTimeout) {
          cursorOpacity.current = Math.max(0, cursorOpacity.current - dt * 2);
        } else if (timeSinceMove < 100) {
          cursorOpacity.current = Math.min(1, cursorOpacity.current + dt * 5);
        }
      } else {
        cursorOpacity.current = 1;
      }

      // Clear canvas
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Transform function
      const transform = (x: number, y: number) => transformToCanvas(x, y, canvas.width, canvas.height, videoTransform);

      // Draw ripples
      if (clickEffect === 'ripple') {
        ripples.current = ripples.current.filter(ripple =>
          drawRipple(ctx, ripple, now, cursorScale, cursorRippleColor, transform)
        );
      }

      // Draw circle highlights
      if (clickEffect === 'circle') {
        circleHighlights.current = circleHighlights.current.filter(circle =>
          drawCircleHighlight(ctx, circle, now, cursorScale, cursorHighlightColor, transform)
        );
      }

      // Draw cursor
      if (targetCursor.current && cursorOpacity.current > 0.01 && !hideCursor) {
        const smoothX = frozenCursorPosition.current?.x ?? cursorSpring.current.x.value;
        const smoothY = frozenCursorPosition.current?.y ?? cursorSpring.current.y.value;
        const { x: canvasX, y: canvasY } = transform(smoothX, smoothY);

        // Horizontal velocity for rotation
        const dx = smoothX - lastXPosition.current;
        lastXPosition.current = smoothX;
        horizontalVelocity.current = horizontalVelocity.current * 0.85 + dx * 0.15;

        let rotation = cursorRotation;
        if (rotateCursorWhileMoving) {
          const maxRotation = 30 * (rotationIntensity / 100);
          rotation += Math.max(-maxRotation, Math.min(maxRotation, horizontalVelocity.current * 2));
        }

        // Update trail
        if (cursorTrailEnabled) {
          trailHistory.current.push({ x: canvasX, y: canvasY, time: now });
          while (trailHistory.current.length > cursorTrailLength) trailHistory.current.shift();
        } else {
          trailHistory.current = [];
        }

        ctx.save();
        ctx.globalAlpha = cursorOpacity.current;

        // Draw trail
        if (cursorTrailEnabled) {
          drawTrail(ctx, trailHistory.current, cursorScale, cursorColor, cursorTrailOpacity, cursorOpacity.current);
        }

        // Draw cursor
        drawCursor({
          ctx,
          canvasX,
          canvasY,
          cursorStyle: effectiveCursorStyle,
          cursorScale,
          cursorColor,
          cursorShadowIntensity,
          rotation,
          isClicking: targetCursor.current.isClicking,
          cursorHighlightColor,
          clickEffect,
          opacity: cursorOpacity.current
        });

        ctx.restore();
      }

      animationFrame = requestAnimationFrame(animate);
    };

    animate();
    return () => { if (animationFrame) cancelAnimationFrame(animationFrame); };
  }, [
    videoRef, mouseEvents, visible, videoWidth, videoHeight,
    canvasSize.width, canvasSize.height, cursorScale, cursorSmoothing,
    hideCursorWhenIdle, idleTimeout, cursorSpeedPreset, cursorColor,
    cursorHighlightColor, cursorRippleColor, cursorShadowIntensity,
    cursorTrailEnabled, cursorTrailLength, cursorTrailOpacity,
    spawnRipple, spawnCircleHighlight, cursorStyle, hideCursor,
    alwaysUsePointer, effectiveCursorStyle, loopCursorPosition,
    clickEffect, cursorRotation, rotateCursorWhileMoving,
    rotationIntensity, stopCursorAtEnd, stopCursorDuration, duration,
    videoTransform
  ]);

  if (!visible) return null;

  return (
    <div
      ref={containerRef}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
        zIndex: 10
      }}
    >
      <canvas
        ref={canvasRef}
        style={{ width: '100%', height: '100%' }}
        width={canvasSize.width}
        height={canvasSize.height}
      />
    </div>
  );
};

export default MouseCursorOverlay;
