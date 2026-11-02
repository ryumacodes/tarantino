import { describe, expect, it } from 'vitest';
import { calculateTimelineLayout, fitTimelineZoom, stepTimelineZoom } from './utils';

describe('calculateTimelineLayout', () => {
  it('fills the usable viewport for a short recording', () => {
    const layout = calculateTimelineLayout(10_000, 1_200, 140, 1);

    expect(layout.mediaWidth).toBe(1_060);
    expect(layout.tracksWidth).toBe(1_200);
    expect(layout.pixelsPerMs).toBeCloseTo(0.106);
  });

  it('adds the track header outside a scrolling timeline', () => {
    const layout = calculateTimelineLayout(20_000, 1_200, 140, 1);

    expect(layout.mediaWidth).toBe(2_000);
    expect(layout.tracksWidth).toBe(2_140);
    expect(layout.pixelsPerMs).toBe(0.1);
  });

  it('reflows to a resized window without reducing the selected zoom', () => {
    const layout = calculateTimelineLayout(10_000, 900, 140, 1.5);

    expect(layout.mediaWidth).toBeCloseTo(1_500);
    expect(layout.tracksWidth).toBeCloseTo(1_640);
    expect(layout.pixelsPerMs).toBeCloseTo(0.15);
  });
});

it.each([100,3000,3600000])('zooms immediately and fits a %sms recording',duration=>{
  const fit=fitTimelineZoom(duration,1060);
  const zoom=stepTimelineZoom(fit,'in',duration,1060);
  expect(calculateTimelineLayout(duration,1200,140,zoom).mediaWidth).toBeCloseTo(1590);
  expect(stepTimelineZoom(zoom,'out',duration,1060)).toBeCloseTo(fit);
  expect(calculateTimelineLayout(duration,1200,140,fit).mediaWidth).toBeCloseTo(1060);
});
it('starts zooming from the displayed fit when the stored zoom is below it',()=>{
  expect(stepTimelineZoom(1,'in',3000,1060)).toBeCloseTo(5.3);
});
