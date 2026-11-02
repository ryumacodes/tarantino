import { expect,it } from 'vitest';
import { displayZoomView } from './displayZoomView';

it('samples a fixed viewport with clamped source centers at zoom edges',()=>{
  expect(displayZoomView(1,0,1)).toEqual({x:0,y:0,width:1,height:1});
  expect(displayZoomView(2,0.25,0.75)).toEqual({x:0,y:0.5,width:0.5,height:0.5});
  expect(displayZoomView(4,0,1)).toEqual({x:0,y:0.75,width:0.25,height:0.25});
});

it('maps source points through the same crop and zoom interval',()=>{
  const view=displayZoomView(2,0.75,0.25);
  // A source point at the selected center maps to the middle of the fixed plane.
  expect((0.75-view.x)/view.width).toBe(0.5);
  expect((0.25-view.y)/view.height).toBe(0.5);
  // Previous mesh translation (scale-1) placed this at 0.75 instead.
  expect((0.5-view.x)/view.width).toBe(0);
});
