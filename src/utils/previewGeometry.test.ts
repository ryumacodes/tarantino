import { describe, expect, it } from 'vitest';
import { previewGeometry } from './previewGeometry';

describe('preview output geometry', () => {
  it('fits widescreen content into a portrait export without stretching', () => {
    const geometry = previewGeometry({ output: { width: 608, height: 1080 },
      source: { width: 1920, height: 1080 }, viewport: { width: 608, height: 1080 }, padding: 0 });
    expect(geometry).toEqual({ canvasWidth: 608, canvasHeight: 1080, planeWidth: 608, planeHeight: 342 });
  });
  it('uses custom output geometry and padding before scaling to the viewport', () => {
    const geometry = previewGeometry({ output: { width: 1000, height: 1000 },
      source: { width: 1920, height: 1080 }, viewport: { width: 500, height: 300 }, padding: 10 });
    expect(geometry).toEqual({ canvasWidth: 300, canvasHeight: 300, planeWidth: 240, planeHeight: 135 });
  });
  it('matches native desktop window proportions', () => {
    const geometry = previewGeometry({ output: { width: 1920, height: 1080 },
      source: { width: 1748, height: 1418 }, viewport: { width: 1920, height: 1080 }, padding: 0,
      desktop: { screen: { width: 1470, height: 956 }, window: { width: 874, height: 709 } } });
    expect(geometry.planeWidth).toBe(986);
    expect(geometry.planeHeight).toBe(800);
  });
});
