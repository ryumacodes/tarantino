import {expect,it} from 'vitest';
import {maskCoverSampling} from './coverSampling';
const full={x:0,y:0,width:1,height:1};
it('insets the sampling lattice and bounds work to 256 samples per quadrant',()=>{
 for(const width of [1,7,31,1920,7680])for(const height of [1,9,99,1080,4320])for(const levels of [1,4,13]){
  const plan=maskCoverSampling(full,full,{width,height},levels)!;
  expect(plan.tapsPerQuadrant.x*plan.tapsPerQuadrant.y).toBeLessThanOrEqual(256);
  expect(plan.level).toBeLessThan(levels);
  expect(plan.lattice.x).toBeGreaterThanOrEqual(0);
  expect(plan.lattice.x+plan.lattice.width).toBeLessThanOrEqual(width);
  expect(plan.lattice.y+plan.lattice.height).toBeLessThanOrEqual(height);
 }
});
it('maps cropped UV space and clips partial regions',()=>{
 const result=maskCoverSampling({x:.2,y:.2,width:.4,height:.4},{x:.4,y:.4,width:.4,height:.4},{width:100,height:100},1)!;
 expect(result.lattice.x).toBe(1.5);expect(result.lattice.y).toBe(1.5);
 expect(result.lattice.width).toBeCloseTo(47);
 expect(maskCoverSampling({x:2,y:2,width:1,height:1},full,{width:100,height:100},1)).toBeNull();
});
it('rejects invalid inputs instead of producing unbounded loops or sample coordinates',()=>{
 expect(()=>maskCoverSampling(full,full,{width:100,height:100},Infinity)).toThrow();
 expect(()=>maskCoverSampling(full,{...full,width:0},{width:100,height:100},1)).toThrow();
});
