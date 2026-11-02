import { describe, expect, it } from 'vitest';
import { previewCameraPose as pose, previewCameraVelocity as velocity } from './previewCamera';

describe('preview camera motion',()=>{
  it('detects zoom without panning and scales with clip speed',()=>{
    const before=pose(1.9,.5,.5,true,8,4),now=pose(2,.5,.5,true,8,4);
    expect(velocity(now,before,8,4,120,1,true)).toEqual({scale:expect.closeTo(6),x:0,y:0});
    expect(velocity(now,before,8,4,120,2,true).scale).toBeCloseTo(12);
  });
  it('does not invent pan when a display view is clamped at an edge',()=>{
    const before=pose(2,.05,.5,false,8,4),now=pose(2,.1,.5,false,8,4);
    expect(velocity(now,before,8,4,120,1,true)).toEqual({scale:0,x:0,y:0});
  });
  it('measures window edge-anchor movement, including pan caused by zoom',()=>{
    const before=pose(1.5,.1,.8,true,8,4),now=pose(1.51,.1,.8,true,8,4);
    const motion=velocity(now,before,8,4,120,1,true);
    expect(motion.x).toBeCloseTo(-(now.offsetX-before.offsetX)/8*1200);
    expect(motion.y).toBeCloseTo((now.offsetY-before.offsetY)/4*1200);
    expect(motion.scale).toBeGreaterThan(0);
  });
  it('clears blur across edits, gaps, pauses and first frames',()=>{
    const before=pose(1,.5,.5,true,8,4),now=pose(3,.9,.1,true,8,4);
    expect(velocity(now,before,8,4,120,1,false)).toEqual({scale:0,x:0,y:0});
  });
  it('keeps velocity independent of canvas dimensions',()=>{
    const a=velocity(pose(2,.6,.6,true,8,4),pose(1.98,.59,.59,true,8,4),8,4,120,1,true);
    const b=velocity(pose(2,.6,.6,true,80,40),pose(1.98,.59,.59,true,80,40),80,40,120,1,true);
    expect(a.x).toBeCloseTo(b.x);expect(a.y).toBeCloseTo(b.y);expect(a.scale).toBe(b.scale);
  });
});
