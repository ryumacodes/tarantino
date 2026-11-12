import { expect, it } from 'vitest';
import { cropSourcePoint, normalizedCrop, resolveSourceCrop } from './sourceCrop';

it('preserves fractional source geometry and preserves positions outside the crop', () => {
  const crop = resolveSourceCrop({ x:0.25,y:0.1,width:0.5,height:0.8 },1919,1079);
  expect(crop).toMatchObject({ x:479.75,y:107.9,width:959.5,height:863.2 });
  for (const [x,y,expected] of [[479.75,107.9,0],[1439.25,971.1,1]]) {
    const point = cropSourcePoint(crop,x/1919,y/1079);
    expect(point.x).toBeCloseTo(expected,12);
    expect(point.y).toBeCloseTo(expected,12);
  }
  expect(cropSourcePoint(crop,0,0).x).toBeLessThan(0);
  expect(cropSourcePoint(crop,1,1).y).toBeGreaterThan(1);
  expect(resolveSourceCrop(normalizedCrop(crop),1919,1079)).toEqual(crop);
});

it('keeps a full-source crop identical and accepts subpixel crops while rejecting invalid rectangles', () => {
  const full = { x:0,y:0,width:1,height:1 };
  expect(cropSourcePoint(resolveSourceCrop(full,1920,1080),0.25,0.75)).toEqual({x:0.25,y:0.75});
  for (const value of [{x:-0.1},{x:NaN},{width:0},{width:1.1}]) {
    expect(() => resolveSourceCrop({...full,...value},100,100)).toThrow();
  }
  expect(resolveSourceCrop({...full,width:0.00001},100,100).width).toBeCloseTo(0.001,12);
  expect(() => resolveSourceCrop({...full,x:1,width:1e-10},100,100)).toThrow();
  expect(() => resolveSourceCrop(full,0,1080)).toThrow();
});
