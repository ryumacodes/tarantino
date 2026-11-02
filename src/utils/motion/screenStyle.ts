import { SPRING_PRESETS } from '../../stores/editor/constants';
import type { VisualSettings } from '../../stores/editor/types';
import type { SpringConfig } from './spring';

export const SCREEN_MOTION_STYLES = {
  focused: { tension: 250, friction: 40, mass: 2.25 },
  smooth: { tension: 170, friction: 50, mass: 3 },
} as const;

export const SCREEN_MOTION_OPTIONS = [
  { preset: 'focused', label: 'Focused', description: 'Direct, responsive framing' },
  { preset: 'smooth', label: 'Smooth', description: 'Gentle, flowing framing' },
] as const;

export function resolveScreenSpring(settings: Pick<VisualSettings, 'screenMotionStyle' | 'zoomSpeedPreset'>): SpringConfig {
  return settings.screenMotionStyle
    ? SCREEN_MOTION_STYLES[settings.screenMotionStyle]
    : SPRING_PRESETS[settings.zoomSpeedPreset ?? 'mellow'];
}
