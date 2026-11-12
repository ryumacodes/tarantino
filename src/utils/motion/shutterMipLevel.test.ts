import {expect, it} from 'vitest';
import {shutterMipLevel} from './shutterMipLevel';

it('keeps static mip selection and bounds a long shutter without building the full chain', () => {
  expect(shutterMipLevel([512, 512], [64, 64, 512, 512], [64, 64, 512, 512], [128, 128], 16, 4)).toBe(1);
  expect(shutterMipLevel([512, 512], [127.5, 127.5, 512, 512], [-.5, 127.5, 512, 512], [128, 128], 16, 4)).toBe(4);
});

it('covers tap footprints across clipped segments, zooms, pans and both quality modes', () => {
  const source = [1919, 1079] as const, output = [960, 540] as const;
  for (const [budget, span] of [[6, 16], [16, 4]]) for (const zoom of [.25, 1, 2, 8]) {
    const before = [355.2, 214.7, 800 * zoom, 450 * zoom] as const;
    const now = [735.1, -25.2, 900 / zoom, 600 / zoom] as const;
    const level = shutterMipLevel(source, before, now, output, budget, span);
    const texelsPerPixel = Math.max(source[0] / before[2], source[1] / before[3]);
    for (let x = 0; x <= 960; x += 60) for (let y = 0; y <= 540; y += 45) {
      const travel = Math.hypot(...source.map((size, axis) => {
        const pixel = axis ? y : x;
        return ((pixel - before[axis]) / before[axis + 2] - (pixel - now[axis]) / now[axis + 2]) * size;
      }));
      for (const fraction of [.001, .1, .5, 1]) {
        const distance = travel * fraction;
        const taps = Math.max(1, Math.min(budget, Math.ceil(distance / texelsPerPixel / span)));
        const needed = Math.floor(Math.max(0, Math.log2(Math.max(distance / taps, texelsPerPixel, 1)))) + 1;
        expect(level).toBeGreaterThanOrEqual(needed);
      }
    }
  }
});
