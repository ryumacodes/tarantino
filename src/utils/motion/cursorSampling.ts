import { CAMERA_SIMULATION_FPS, cameraFrameIndex } from './zoomTrajectory';

/** Segment bounds from the native edit plan are sorted and non-overlapping. */
export function cursorSegmentIndex(segments: readonly number[][], timeMs: number): number {
  let low = 0, high = segments.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (segments[middle][0] <= timeMs) low = middle + 1;
    else high = middle;
  }
  const index = low - 1;
  return index >= 0 && timeMs < segments[index][1] ? index : -1;
}

export function sampleCursorFrame(frames: readonly number[][], timeMs: number,
  segments?: readonly number[][]): number[] {
  const index = cameraFrameIndex(timeMs, frames.length);
  const current = frames[index];
  const next = frames[index + 1];
  if (!current || !next || !Number.isFinite(timeMs) || timeMs <= 0) return current;
  const position = timeMs * CAMERA_SIMULATION_FPS / 1000;
  const fraction = Math.abs(position - Math.round(position)) < 1e-7 ? 0 : position - index;
  if (fraction === 0) return current;
  if (segments && cursorSegmentIndex(segments, timeMs) < 0) {
    return current.map(() => 0);
  }
  const result = [...current];
  for (const channel of [0, 1, 2, 7, 8, 9]) {
    if (channel < current.length && channel < next.length) {
      result[channel] = current[channel] + (next[channel] - current[channel]) * fraction;
    }
  }
  return result;
}
