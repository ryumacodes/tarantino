import type { ZoomBlock, LoupeStyle } from '../../stores/editor/types';

const KEY = 'tarantino.zoomStyle';
type ZoomStyle = Pick<ZoomBlock, 'zoom_factor' | 'instant_animation' | 'loupe' | 'inactive_loupe'>;

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object';
}

function isLens(value: unknown): value is LoupeStyle {
  return isObject(value) && typeof value.glass === 'boolean'
    && ['radius', 'bevel', 'chromatic_aberration', 'shadow_alpha']
      .every(key => typeof value[key] === 'number' && Number.isFinite(value[key]));
}

function pickStyle(block: ZoomBlock): ZoomStyle {
  return { zoom_factor: block.zoom_factor, instant_animation: block.instant_animation ?? false,
    loupe: block.loupe ? { ...block.loupe } : null,
    ...(!block.loupe && block.inactive_loupe && {inactive_loupe: {...block.inactive_loupe}}) };
}

export function readZoomStyle(): ZoomStyle {
  const fallback = { zoom_factor: 2, instant_animation: false, loupe: null };
  try {
    const value: unknown = JSON.parse(localStorage.getItem(KEY) ?? 'null');
    if (!isObject(value) || typeof value.zoom_factor !== 'number' || !Number.isFinite(value.zoom_factor)
      || value.zoom_factor <= 0 || typeof value.instant_animation !== 'boolean') return fallback;
    const lens = value.loupe;
    if (lens !== null && !isLens(lens)) return fallback;
    if (value.inactive_loupe !== undefined && !isLens(value.inactive_loupe)) return fallback;
    const loupe: LoupeStyle | null = lens === null ? null : {
      radius: lens.radius, glass: lens.glass, bevel: lens.bevel,
      chromatic_aberration: lens.chromatic_aberration, shadow_alpha: lens.shadow_alpha,
    };
    return { zoom_factor: value.zoom_factor, instant_animation: value.instant_animation, loupe,
      ...(value.inactive_loupe && {inactive_loupe:{...value.inactive_loupe}}) };
  } catch { return fallback; }
}

export function rememberEditedZoomStyle(before: ZoomBlock, after: ZoomBlock) {
  const previous = JSON.stringify(pickStyle(before));
  const next = JSON.stringify(pickStyle(after));
  if (previous === next) return;
  try { localStorage.setItem(KEY, next); } catch { /* Editing remains available without preference storage. */ }
}
