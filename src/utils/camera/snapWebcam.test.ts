import { expect, it } from 'vitest';
import { snapWebcam } from './snapWebcam';
const corners=[{x:0.1,y:0.1},{x:0.9,y:0.1},{x:0.1,y:0.9},{x:0.9,y:0.9}];
it('attracts nearby positions to each actual corner', () => {
  for(const corner of corners) {
    const nearby={x:corner.x+(corner.x<0.5?0.03:-0.03),y:corner.y+(corner.y<0.5?0.02:-0.02)};
    expect(snapWebcam(nearby,corners,0.09)).toEqual(corner);
  }
});
it('keeps free positions outside the radius and chooses the closest candidate', () => {
  const free={x:0.5,y:0.5};
  expect(snapWebcam(free,corners,0.09)).toBe(free);
  expect(snapWebcam(free,[],0.09)).toBe(free);
  expect(snapWebcam({x:0.2,y:0.2},[{x:0,y:0},{x:0.21,y:0.21}],0.5)).toEqual({x:0.21,y:0.21});
});
