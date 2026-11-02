import { expect, it } from 'vitest';
import { resolveCursorSpring, CURSOR_MOTION_STYLES } from './cursorStyle';
import { springStep } from './spring';

it('bypasses smoothing in None mode even when a legacy speed is set', () => {
  const config = resolveCursorSpring({ cursorMotionStyle: 'none', cursorSpeedPreset: 'slow' });
  expect(springStep(0.2, 0.8, 5, config, 1 / 60)).toEqual({ value: 0.8, velocity: 0 });
});

it('keeps old settings usable and new styles independent of the legacy speed', () => {
  expect(resolveCursorSpring({ cursorSpeedPreset: 'slow' }).tension).toBe(120);
  expect(resolveCursorSpring({ cursorMotionStyle: 'smooth', cursorSpeedPreset: 'slow' }))
    .toEqual(resolveCursorSpring({ cursorMotionStyle: 'smooth', cursorSpeedPreset: 'rapid' }));
});

it.each([24, 30, 60, 120])('produces distinct, bounded cursor responses at %s fps', fps => {
  const positions: number[] = [];
  for (const name of ['smooth', 'medium', 'rapid'] as const) {
    let state = { value: 0, velocity: 0 };
    for (let frame = 0; frame < fps * 2; frame++) {
      state = springStep(state.value, 1, state.velocity, CURSOR_MOTION_STYLES[name], 1 / fps);
      expect(state.value).toBeGreaterThanOrEqual(0);
      expect(state.value).toBeLessThan(1.1);
      if (frame === Math.round(fps / 4)) positions.push(state.value);
    }
    expect(state.value).toBeCloseTo(1, 3);
  }
  expect(new Set(positions.map(x => x.toFixed(3))).size).toBe(3);
});
