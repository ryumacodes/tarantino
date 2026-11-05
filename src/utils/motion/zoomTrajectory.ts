import type { ZoomBlock, ZoomAnalysis, VisualSettings } from '../../stores/editor/types';

export const CAMERA_SIMULATION_FPS = 120;
export type CameraFrame = [scale: number, centerX: number, centerY: number,
  rectX?: number, rectY?: number, rectWidth?: number, rectHeight?: number];
const IDENTITY: CameraFrame = [1, 0.5, 0.5];

export function cameraFrameIndex(timeMs: number, frameCount: number): number {
  if (!Number.isFinite(timeMs) || frameCount <= 0) return 0;
  return Math.max(0, Math.min(frameCount - 1, Math.floor(timeMs * CAMERA_SIMULATION_FPS / 1000 + 1e-7)));
}

export function sampleCameraFrame(frames: readonly CameraFrame[], timeMs: number): CameraFrame {
  return frames[cameraFrameIndex(timeMs, frames.length)] ?? IDENTITY;
}

/** Shared IPC shape for preview and export. */
export function exportZoomBlock(block: ZoomBlock, leadInMs = 0) {
  return {
    is_manual: block.is_manual,
    is_disabled: block.is_disabled ?? false,
    loupe: block.loupe ?? null,
    animation_lead_in_ms: Number.isFinite(leadInMs) ? Math.round(Math.max(0, Math.min(2000, leadInMs))) : 0,
    instant_animation: block.instant_animation ?? false,
    start_time_ms: Math.max(0, Math.round(block.start_time)),
    end_time_ms: Math.max(0, Math.round(block.end_time)),
    zoom_level: block.zoom_factor,
    center_x: block.center_x,
    center_y: block.center_y,
    kind: block.kind ?? 'click',
    zoom_in_speed: block.zoom_in_speed ?? null,
    zoom_out_speed: block.zoom_out_speed ?? null,
    timing_adjusted: block.timing_adjusted ?? false,
    centers: (block.centers ?? []).map(center => ({ ...center, time: Math.max(0, Math.round(center.time)) })),
  };
}

/** The same effect selection feeds camera/magnifier preview and export. */
export function exportZoomBlocks(analysis: ZoomAnalysis | null, settings: Pick<VisualSettings, 'zoomEffectsEnabled' | 'zoomLeadInMs'>) {
  return settings.zoomEffectsEnabled === false ? []
    : (analysis?.zoom_blocks ?? []).map(block => exportZoomBlock(block, settings.zoomLeadInMs));
}
