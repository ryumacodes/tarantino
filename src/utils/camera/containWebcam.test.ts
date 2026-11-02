import { expect, it } from 'vitest';
import { containWebcam } from './containWebcam';

it('keeps default camera inside a wide canvas', () => {
  expect(containWebcam(.895, .895, .15, 1920, 1080)).toEqual({x:.895, y:1-.15*(1920/1080)/2, size:.15});
});
it('contains all four edges and oversized cameras across aspect ratios', () => {
  for (const [width,height] of [[1920,1080],[1080,1920],[2560,1080],[1000,1000]]) {
    for (const x of [0,.5,1]) for (const y of [0,.5,1]) for (const size of [.15,.5,1]) {
      const c=containWebcam(x,y,size,width,height);
      const halfX=c.size*width/2, halfY=halfX;
      expect(c.x*width-halfX).toBeGreaterThanOrEqual(-1e-8);
      expect(c.y*height-halfY).toBeGreaterThanOrEqual(-1e-8);
      expect(c.x*width+halfX).toBeLessThanOrEqual(width+1e-8);
      expect(c.y*height+halfY).toBeLessThanOrEqual(height+1e-8);
    }
  }
});
