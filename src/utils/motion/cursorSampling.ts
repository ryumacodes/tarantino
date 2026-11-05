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
  if (segments) {
    const start = index * 1000 / CAMERA_SIMULATION_FPS;
    const end = (index + 1) * 1000 / CAMERA_SIMULATION_FPS;
    const segment = segments.find(([a, b]) => a <= timeMs && timeMs < b);
    if (!segment || start < segment[0] || end >= segment[1]) return current;
  }
  const result = [...current];
  for (const channel of [0, 1, 2, 7, 8, 9]) {
    if (channel < current.length && channel < next.length) {
      result[channel] = current[channel] + (next[channel] - current[channel]) * fraction;
    }
  }
  return result;
}
