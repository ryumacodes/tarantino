import type { ZoomBlock } from '../../stores/editor/types';
import { cropSourcePoint, type PixelCrop } from '../sourceCrop';

export function zoomPickerCenter(block: Pick<ZoomBlock, 'is_manual' | 'center_x' | 'center_y'>, crop: PixelCrop | null) {
  return crop && !block.is_manual ? cropSourcePoint(crop, block.center_x, block.center_y)
    : { x: block.center_x, y: block.center_y };
}

export function zoomCenterBounds(zoom: number, captureMode: 'display' | 'window' | null, magnifier = false) {
  const inset = captureMode === 'window' || magnifier ? 0 : 0.5 / zoom;
  return { min: inset, max: 1 - inset };
}

export function manualZoomCenterUpdate(
  block: Pick<ZoomBlock, 'is_manual' | 'center_x' | 'center_y'>,
  zoom: number, captureMode: 'display' | 'window' | null, magnifier = false,
) {
  if (!block.is_manual) return {};
  const bounds = zoomCenterBounds(zoom, captureMode, magnifier);
  const clamp = (value: number) => Math.max(bounds.min, Math.min(bounds.max, value));
  return { center_x: clamp(block.center_x), center_y: clamp(block.center_y) };
}
