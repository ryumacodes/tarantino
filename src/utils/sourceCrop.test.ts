import { expect, it } from 'vitest';
import { cropSourcePoint, normalizedCrop, resolveSourceCrop } from './sourceCrop';

it('matches native edge quantization and preserves positions outside the crop', () => {
  const crop = resolveSourceCrop({ x:0.25,y:0.1,width:0.5,height:0.8 },1919,1079);
  expect(crop).toMatchObject({ x:480,y:108,width:959,height:863 });
  for (const [x,y,expected] of [[480,108,0],[1439,971,1]]) {
    const point = cropSourcePoint(crop,x/1919,y/1079);
    expect(point.x).toBeCloseTo(expected,12);
    expect(point.y).toBeCloseTo(expected,12);
  }
  expect(cropSourcePoint(crop,0,0).x).toBeLessThan(0);
  expect(cropSourcePoint(crop,1,1).y).toBeGreaterThan(1);
  expect(resolveSourceCrop(normalizedCrop(crop),1919,1079)).toEqual(crop);
});

it('keeps a full-source crop identical and rejects invalid or subpixel-empty rectangles', () => {
  const full = { x:0,y:0,width:1,height:1 };
  expect(cropSourcePoint(resolveSourceCrop(full,1920,1080),0.25,0.75)).toEqual({x:0.25,y:0.75});
  for (const value of [{x:-0.1},{x:NaN},{width:0},{width:1.1},{width:0.00001}]) {
    expect(() => resolveSourceCrop({...full,...value},100,100)).toThrow();
  }
  expect(() => resolveSourceCrop(full,0,1080)).toThrow();
});
