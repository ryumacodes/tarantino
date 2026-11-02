import { SPRING_PRESETS } from '../../stores/editor/constants';
import type { VisualSettings } from '../../stores/editor/types';
import type { SpringConfig } from './spring';

export const CURSOR_MOTION_STYLES = {
  smooth: { tension: 470, friction: 70, mass: 3 },
  medium: { tension: 340, friction: 60, mass: 3 },
  rapid: { tension: 530, friction: 40, mass: 1 },
  none: { tension: 1, friction: 0, mass: 0 },
} as const;

export const CURSOR_MOTION_OPTIONS = [
  { preset: 'smooth', label: 'Smooth', description: 'Soft, flowing movement' },
  { preset: 'medium', label: 'Medium', description: 'Gentle, controlled movement' },
  { preset: 'rapid', label: 'Rapid', description: 'Fast, responsive movement' },
  { preset: 'none', label: 'None', description: 'Follow the recorded position directly' },
] as const;

export function resolveCursorSpring(settings: Pick<VisualSettings, 'cursorMotionStyle' | 'cursorSpeedPreset'>): SpringConfig {
  return settings.cursorMotionStyle
    ? CURSOR_MOTION_STYLES[settings.cursorMotionStyle]
    : SPRING_PRESETS[settings.cursorSpeedPreset ?? 'mellow'];
}
