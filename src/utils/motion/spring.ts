export interface SpringConfig { tension: number; friction: number; mass: number }
export interface SpringState { value: number; velocity: number }

/** Damped motion in seconds, integrated independently of display/output cadence.
 * Keep equations and stopping thresholds aligned with cursor_renderer/spring.rs.
 * Substeps prevent stiff following springs from exploding at low frame rates.
 */
export function springStep(
  current: number, target: number, velocity: number, config: SpringConfig, dt: number,
): SpringState {
  if (!Number.isFinite(dt) || dt <= 0) return { value: current, velocity };
  const { tension, mass } = config;
  let { friction } = config;
  if (mass === 0) return { value: target, velocity: 0 };
  if (![current, target, velocity, tension, friction, mass].every(Number.isFinite)
      || tension <= 0 || friction < 0 || mass < 0) return { value: current, velocity };
  // Brake only when momentum toward the target exceeds its natural approach speed.
  // Do not clip positions or erase velocity when a target reverses direction.
  const distance = target - current;
  if (velocity * distance > 0 && Math.abs(velocity) > Math.abs(distance) * Math.sqrt(tension / mass)) {
    friction = Math.max(friction, 2 * Math.sqrt(tension * mass));
  }
  const maxStep = Math.min(0.001, 0.25 / Math.max(Math.sqrt(tension / mass), friction / mass));
  let remaining = dt;
  while (remaining > 1e-12) {
    const step = Math.min(maxStep, remaining);
    velocity += (-tension * (current - target) - friction * velocity) / mass * step;
    current += velocity * step;
    if (Math.abs(current - target) < 0.0001 && Math.abs(velocity) < 0.0001) {
      return { value: target, velocity: 0 };
    }
    remaining -= step;
  }
  return { value: current, velocity };
}
