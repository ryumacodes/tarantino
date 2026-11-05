import { describe, expect, it } from 'vitest';
import { contentInset, previewGeometry } from './previewGeometry';

describe('preview output geometry', () => {
  it('fits widescreen content into a portrait export without stretching', () => {
    const geometry = previewGeometry({ output: { width: 608, height: 1080 },
      source: { width: 1920, height: 1080 }, viewport: { width: 608, height: 1080 }, padding: 0 });
    expect(geometry).toEqual({ canvasWidth: 608, canvasHeight: 1080, planeWidth: 608, planeHeight: 342 });
  });
  it('uses custom output geometry and padding before scaling to the viewport', () => {
    const geometry = previewGeometry({ output: { width: 1000, height: 1000 },
      source: { width: 1920, height: 1080 }, viewport: { width: 500, height: 300 }, padding: 10 });
    expect(geometry).toEqual({ canvasWidth: 300, canvasHeight: 300, planeWidth: 250.2, planeHeight: 140.4 });
  });
  it('matches native desktop window proportions', () => {
    const geometry = previewGeometry({ output: { width: 1920, height: 1080 },
      source: { width: 1748, height: 1418 }, viewport: { width: 1920, height: 1080 }, padding: 0,
      desktop: { screen: { width: 1470, height: 956 }, window: { width: 874, height: 709 } } });
    expect(geometry.planeWidth).toBe(986);
    expect(geometry.planeHeight).toBe(800);
  });
});

describe('content padding', () => {
  it.each([
    [1920, 1080, 10, 90],
    [608, 1080, 10, 50],
    [1000, 1000, 10, 83],
    [1920, 1080, 50, 270],
    [1920, 1080, 0, 0],
  ])('insets %i × %i at %i percent by %i pixels on each side', (width, height, padding, inset) => {
    expect(contentInset(width, height, padding)).toBe(inset);
    expect(contentInset(height, width, padding)).toBe(inset);
  });
});
