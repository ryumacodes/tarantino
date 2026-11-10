import type { ZoomBlock } from '../../stores/editor/types';

export function zoomPresentationUpdate(block: ZoomBlock, presentation: 'screen' | 'magnifier'): Partial<ZoomBlock> {
  if (presentation === 'magnifier') {
    return { loupe: { ...(block.loupe ?? block.inactive_loupe ?? {
      radius: 0.35, glass: false, bevel: 0.2, chromatic_aberration: 0.5, shadow_alpha: 0.35,
    }) }, inactive_loupe: undefined };
  }
  return { loupe: null,
    ...(block.loupe && { inactive_loupe: { ...block.loupe } }) };
}
