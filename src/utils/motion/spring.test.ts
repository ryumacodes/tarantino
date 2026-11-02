import { describe, expect, it } from 'vitest';
import { springStep, type SpringConfig } from './spring';

const follow: SpringConfig = { tension: 520, friction: 52, mass: 1 };
function advance(config: SpringConfig, steps: number[]) {
  let state = { value: 0, velocity: 0 };
  for (const dt of steps) state = springStep(state.value, 1, state.velocity, config, dt);
  return state;
}
describe('motion integration', () => {
  it.each([24, 30, 60, 120])('keeps follow motion bounded and settled at %s FPS', fps => {
    let state = { value: 0, velocity: 0 };
    for (let i = 0; i < 2 * fps; i++) {
      state = springStep(state.value, 1, state.velocity, follow, 1 / fps);
      expect(state.value).toBeGreaterThanOrEqual(0);
      expect(state.value).toBeLessThanOrEqual(1);
    }
    expect(state.value).toBeCloseTo(1, 4);
    expect(Math.abs(state.velocity)).toBeLessThan(0.0001);
  });
  it('agrees with the independent critical-damping solution', () => {
    const state = advance({ tension: 100, friction: 20, mass: 1 }, [0.5]);
    expect(Math.abs(state.value - (1 - 6 * Math.exp(-5)))).toBeLessThan(0.001);
    expect(Math.abs(state.velocity - 50 * Math.exp(-5))).toBeLessThan(0.003);
  });
  it('preserves elapsed time across dropped and irregular display frames', () => {
    const regular = advance(follow, Array(30).fill(1 / 60));
    const irregular = advance(follow, [0.016, 0.1, 0.024, 0.2, 0.16]);
    expect(Math.abs(regular.value - irregular.value)).toBeLessThan(0.0001);
    expect(Math.abs(regular.velocity - irregular.velocity)).toBeLessThan(0.001);
  });
  it('preserves momentum when a target reverses', () => {
    const state = springStep(0.5, 0, 2, follow, 0.001);
    expect(state.value).toBeGreaterThan(0.5);
    expect(state.velocity).toBeGreaterThan(0);
    expect(state.velocity).toBeLessThan(2);
  });
  it('supports immediate motion and ignores invalid time steps', () => {
    expect(springStep(0, 1, 5, { ...follow, mass: 0 }, 0.016)).toEqual({ value: 1, velocity: 0 });
    for (const dt of [0, -1, NaN, Infinity]) {
      expect(springStep(0.4, 1, 2, follow, dt)).toEqual({ value: 0.4, velocity: 2 });
    }
  });
});

describe('adaptive braking', () => {
  const soft = { tension: 100, friction: 2, mass: 1 };
  const critical = { ...soft, friction: 20 };
  it.each([24, 30, 60, 120])('raises damping for a fast approach at %s fps', fps => {
    expect(springStep(0.9, 1, 2, soft, 1 / fps))
      .toEqual(springStep(0.9, 1, 2, critical, 1 / fps));
  });
  it('does not erase momentum when the target moves behind the current position', () => {
    const result = springStep(0.9, 0, 2, soft, 0.001);
    // One physical step with the original damping: a = -100 * .9 - 2 * 2.
    expect(result.velocity).toBeCloseTo(1.906, 12);
    expect(result.value).toBeCloseTo(0.901906, 12);
  });
  it('retains the selected damping during a slow approach', () => {
    const result = springStep(0, 1, 1, soft, 0.001);
    expect(result.velocity).toBeCloseTo(1.098, 12);
  });
  it('never reduces an already overdamped configuration', () => {
    const result = springStep(0.9, 1, 2, { ...soft, friction: 30 }, 0.001);
    expect(result.velocity).toBeCloseTo(1.95, 12);
  });
});
