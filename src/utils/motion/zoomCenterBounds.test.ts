import { expect, it } from 'vitest';
import { manualZoomCenterUpdate, zoomCenterBounds, zoomPickerCenter } from './zoomCenterBounds';
import { cropSourcePoint, resolveSourceCrop } from '../sourceCrop';

it('allows window anchors at either edge when changing zoom or presentation', () => {
  for (const zoom of [1.2, 2, 4]) {
    expect(zoomCenterBounds(zoom, 'window')).toEqual({ min: 0, max: 1 });
    expect(zoomCenterBounds(zoom, 'window', true)).toEqual({ min: 0, max: 1 });
  }
});

it('preserves automatic source targets when changing zoom or leaving magnifier mode', () => {
  const block = { is_manual: false, center_x: 0.9, center_y: 0.1 };
  const crop = resolveSourceCrop({ x: 0.5, y: 0, width: 0.5, height: 1 }, 1920, 1080);
  for (const zoom of [1.2, 2, 4]) {
    const updated = { ...block, ...manualZoomCenterUpdate(block, zoom, 'display') };
    expect(updated).toEqual(block);
    expect(cropSourcePoint(crop, updated.center_x, updated.center_y)).toEqual({ x: 0.8, y: 0.1 });
  }
});

it('constrains manual display anchors without changing window or magnifier anchors', () => {
  const block = { is_manual: true, center_x: 0.9, center_y: 0.1 };
  expect(manualZoomCenterUpdate(block, 2, 'display')).toEqual({ center_x: 0.75, center_y: 0.25 });
  for (const [mode, magnifier] of [['window', false], ['display', true]] as const) {
    expect(manualZoomCenterUpdate(block, 2, mode, magnifier)).toEqual({ center_x: 0.9, center_y: 0.1 });
  }
});

it('shows automatic source anchors in crop space without remapping manual anchors twice', () => {
  const crop = resolveSourceCrop({ x: 0.5, y: 0, width: 0.5, height: 1 }, 1920, 1080);
  const block = { is_manual: false, center_x: 0.9, center_y: 0.1 };
  expect(zoomPickerCenter(block, crop)).toEqual({ x: 0.8, y: 0.1 });
  expect(zoomPickerCenter({ ...block, is_manual: true }, crop)).toEqual({ x: 0.9, y: 0.1 });
  expect(zoomPickerCenter(block, null)).toEqual({ x: 0.9, y: 0.1 });
  expect(zoomPickerCenter({ ...block, center_x: 0.25 }, crop).x).toBe(-0.5);
});

it('keeps display framing inside the source while allowing magnifier edge targets', () => {
  expect(zoomCenterBounds(2, 'display')).toEqual({ min: 0.25, max: 0.75 });
  expect(zoomCenterBounds(4, 'display')).toEqual({ min: 0.125, max: 0.875 });
  expect(zoomCenterBounds(2, 'display', true)).toEqual({ min: 0, max: 1 });
  expect(zoomCenterBounds(2, null)).toEqual(zoomCenterBounds(2, 'display'));
});
