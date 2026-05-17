import type { CursorStyle, Ripple, CircleHighlight, TrailPosition } from './types';
import { parseColor } from './hooks/useCursorPhysics';

interface DrawCursorParams {
  ctx: CanvasRenderingContext2D;
  canvasX: number;
  canvasY: number;
  cursorStyle: CursorStyle;
  cursorScale: number;
  cursorColor: string;
  cursorShadowIntensity: number;
  rotation: number;
  isClicking: boolean;
  cursorHighlightColor: string;
  clickEffect: 'none' | 'circle' | 'ripple';
  opacity: number;
}

/**
 * Draw arrow cursor shape
 */
export const drawArrowCursor = (
  ctx: CanvasRenderingContext2D,
  canvasX: number,
  canvasY: number,
  fillColor: string,
  strokeColor: string,
  strokeWidth: number,
  cursorScale: number,
  cursorShadowIntensity: number,
  rotation: number,
  dashed: boolean = false
) => {
  ctx.save();
  ctx.translate(canvasX, canvasY);
  ctx.rotate(rotation * Math.PI / 180);

  if (dashed) {
    ctx.setLineDash([2 * cursorScale, 2 * cursorScale]);
  }

  // Arrow path matching SVG: M5,3 L5,19 L9,15 L12,21 L14.5,20 L11.5,14 L17,14 Z
  // Translated so tip is at origin (0,0)
  const traceCursorPath = () => {
    ctx.beginPath();
    ctx.moveTo(0, 0);
    ctx.lineTo(0, 16 * cursorScale);
    ctx.lineTo(4 * cursorScale, 12 * cursorScale);
    ctx.lineTo(7 * cursorScale, 18 * cursorScale);
    ctx.lineTo(9.5 * cursorScale, 17 * cursorScale);
    ctx.lineTo(6.5 * cursorScale, 11 * cursorScale);
    ctx.lineTo(12 * cursorScale, 11 * cursorScale);
    ctx.closePath();
  };

  // Draw shadow first
  const shadowAlpha = (cursorShadowIntensity / 100) * 0.5;
  if (shadowAlpha > 0) {
    ctx.save();
    ctx.fillStyle = `rgba(0, 0, 0, ${shadowAlpha})`;
    const shadowOffset = 2 * cursorScale;
    ctx.translate(shadowOffset, shadowOffset);
    traceCursorPath();
    ctx.fill();
    ctx.restore();
  }

  // Draw cursor
  traceCursorPath();

  if (fillColor !== 'transparent') {
    ctx.fillStyle = fillColor;
    ctx.fill();
  }

  ctx.strokeStyle = strokeColor;
  ctx.lineWidth = strokeWidth;
  ctx.stroke();

  ctx.restore();
};

/**
 * Draw circle cursor style
 */
export const drawCircleCursor = (
  ctx: CanvasRenderingContext2D,
  canvasX: number,
  canvasY: number,
  cursorScale: number,
  cursorShadowIntensity: number,
  rotation: number
) => {
  ctx.save();
  ctx.translate(canvasX, canvasY);
  ctx.rotate(rotation * Math.PI / 180);

  // Shadow
  const shadowAlpha = (cursorShadowIntensity / 100) * 0.5;
  if (shadowAlpha > 0) {
    ctx.beginPath();
    ctx.arc(2 * cursorScale, 2 * cursorScale, 10 * cursorScale, 0, 2 * Math.PI);
    ctx.fillStyle = `rgba(0, 0, 0, ${shadowAlpha})`;
    ctx.fill();
  }

  // Circle cursor
  ctx.beginPath();
  ctx.arc(0, 0, 10 * cursorScale, 0, 2 * Math.PI);
  ctx.fillStyle = 'rgba(128, 128, 128, 0.8)';
  ctx.fill();
  ctx.restore();
};

/**
 * Draw cursor based on style
 */
export const drawCursor = (params: DrawCursorParams) => {
  const {
    ctx,
    canvasX,
    canvasY,
    cursorStyle,
    cursorScale,
    cursorColor,
    cursorShadowIntensity,
    rotation,
    isClicking,
    cursorHighlightColor,
    clickEffect,
    opacity
  } = params;

  const currentCursorColor = isClicking ? cursorHighlightColor : cursorColor;

  ctx.save();
  ctx.globalAlpha = opacity;

  switch (cursorStyle) {
    case 'pointer':
      drawArrowCursor(ctx, canvasX, canvasY, currentCursorColor, '#000000', 1.5 * cursorScale, cursorScale, cursorShadowIntensity, rotation);
      break;

    case 'circle':
      drawCircleCursor(ctx, canvasX, canvasY, cursorScale, cursorShadowIntensity, rotation);
      break;

    case 'filled':
      drawArrowCursor(ctx, canvasX, canvasY, '#000000', '#ffffff', 2 * cursorScale, cursorScale, cursorShadowIntensity, rotation);
      break;

    case 'outline':
      drawArrowCursor(ctx, canvasX, canvasY, 'transparent', '#ffffff', 2 * cursorScale, cursorScale, cursorShadowIntensity, rotation);
      break;

    case 'dotted':
      drawArrowCursor(ctx, canvasX, canvasY, currentCursorColor, '#000000', 1.5 * cursorScale, cursorScale, cursorShadowIntensity, rotation, true);
      break;

    default:
      drawArrowCursor(ctx, canvasX, canvasY, currentCursorColor, '#000000', 1.5 * cursorScale, cursorScale, cursorShadowIntensity, rotation);
  }

  // clickEffect=none means no click visual at all

  ctx.restore();
};

/**
 * Draw ripple effect
 */
export const drawRipple = (
  ctx: CanvasRenderingContext2D,
  ripple: Ripple,
  now: number,
  cursorScale: number,
  rippleColor: string,
  transformFn: (x: number, y: number) => { x: number; y: number }
): boolean => {
  const age = now - ripple.startTime;
  if (age > ripple.duration) return false;

  const progress = age / ripple.duration;
  const radius = 8 + progress * 40; // Expand from 8 to 48
  const alpha = (1 - progress) * 0.6; // Fade from 0.6 to 0

  const { x: canvasX, y: canvasY } = transformFn(ripple.x, ripple.y);
  const rgb = parseColor(rippleColor);

  ctx.save();
  ctx.beginPath();
  ctx.arc(canvasX, canvasY, radius * cursorScale, 0, 2 * Math.PI);
  ctx.strokeStyle = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${alpha})`;
  ctx.lineWidth = 2.5;
  ctx.stroke();

  // Inner ripple
  if (progress < 0.5) {
    const innerAlpha = (0.5 - progress) * 0.4;
    const innerRadius = 4 + progress * 20;
    ctx.beginPath();
    ctx.arc(canvasX, canvasY, innerRadius * cursorScale, 0, 2 * Math.PI);
    ctx.fillStyle = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${innerAlpha})`;
    ctx.fill();
  }

  ctx.restore();
  return true;
};

/**
 * Draw circle highlight effect
 */
export const drawCircleHighlight = (
  ctx: CanvasRenderingContext2D,
  circle: CircleHighlight,
  now: number,
  cursorScale: number,
  highlightColor: string,
  transformFn: (x: number, y: number) => { x: number; y: number }
): boolean => {
  const age = now - circle.startTime;
  if (age > circle.duration) return false;

  const progress = age / circle.duration;
  const alpha = (1 - progress) * 0.8;
  const radius = 20 * cursorScale;

  const { x: canvasX, y: canvasY } = transformFn(circle.x, circle.y);
  const rgb = parseColor(highlightColor);

  ctx.save();
  ctx.beginPath();
  ctx.arc(canvasX, canvasY, radius, 0, 2 * Math.PI);
  ctx.fillStyle = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${alpha * 0.3})`;
  ctx.fill();
  ctx.strokeStyle = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${alpha})`;
  ctx.lineWidth = 2;
  ctx.stroke();
  ctx.restore();

  return true;
};

/**
 * Draw cursor trail
 */
export const drawTrail = (
  ctx: CanvasRenderingContext2D,
  trail: TrailPosition[],
  cursorScale: number,
  cursorColor: string,
  trailOpacity: number,
  cursorOpacity: number
) => {
  if (trail.length <= 1) return;

  const rgb = parseColor(cursorColor);

  for (let i = 0; i < trail.length - 1; i++) {
    const pos = trail[i];
    const progress = i / trail.length;
    const trailAlpha = progress * trailOpacity * cursorOpacity;
    const trailSize = (2 + progress * 4) * cursorScale;

    ctx.beginPath();
    ctx.arc(pos.x, pos.y, trailSize, 0, 2 * Math.PI);
    ctx.fillStyle = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${trailAlpha})`;
    ctx.fill();
  }
};
