import { expect, it } from 'vitest';
import { resolveScreenSpring, SCREEN_MOTION_STYLES } from './screenStyle';
import { springStep } from './spring';

it('retains legacy speed settings and gives explicit styles precedence', () => {
  expect(resolveScreenSpring({ zoomSpeedPreset: 'quick' }).tension).toBe(280);
  expect(resolveScreenSpring({ screenMotionStyle: 'smooth', zoomSpeedPreset: 'rapid' }))
    .toEqual(SCREEN_MOTION_STYLES.smooth);
});

it.each([24, 30, 60, 120])('gives Focused a quicker response than Smooth at %s fps', fps => {
  const positions: number[] = [];
  for (const config of [SCREEN_MOTION_STYLES.focused, SCREEN_MOTION_STYLES.smooth]) {
    let state = { value: 1, velocity: 0 };
    for (let i = 0; i < fps * 3; i++) {
      state = springStep(state.value, 2, state.velocity, config, 1 / fps);
      expect(state.value).toBeGreaterThanOrEqual(1);
      expect(state.value).toBeLessThan(2.02);
      if (i === Math.round(fps / 4)) positions.push(state.value);
    }
    expect(state.value).toBeCloseTo(2, 3);
  }
  expect(positions[0]).toBeGreaterThan(positions[1]);
});
