import { CAMERA_SIMULATION_FPS, cameraFrameIndex } from './zoomTrajectory';

export function sampleCursorFrame(frames: readonly number[][], timeMs: number,
  segments?: readonly number[][]): number[] {
  const index = cameraFrameIndex(timeMs, frames.length);
  const current = frames[index];
  const next = frames[index + 1];
  if (!current || !next || !Number.isFinite(timeMs) || timeMs <= 0) return current;
  const position = timeMs * CAMERA_SIMULATION_FPS / 1000;
  const fraction = Math.abs(position - Math.round(position)) < 1e-7 ? 0 : position - index;
  if (fraction === 0) return current;
  if (segments && !segments.some(([start, end]) => start <= timeMs && timeMs < end)) {
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
