import {expect,it} from 'vitest';
import {captionCorner} from './corners';
it('reduces capsules and zero-radius shapes to circular corner distance',()=>{
 expect(captionCorner(40,24)).toEqual({radius:12,curvature:Array(16).fill(1)});
 expect(captionCorner(100,100,0)).toEqual({radius:0,curvature:Array(16).fill(1)});
 expect(captionCorner(0,10)).toEqual({radius:0,curvature:Array(16).fill(1)});
});
it('is scale invariant and transitions continuously as the available space shrinks',()=>{
 const normal=captionCorner(200,80);
 expect(normal.radius).toBe(24);
 expect(normal.curvature[0]).toBe(1);
 expect(normal.curvature[15]).toBeGreaterThan(1);
 expect(normal).toEqual({radius:24,curvature:captionCorner(400,160,24).curvature});
 let previous=captionCorner(200,24);
 for(let height=24.1;height<=60;height+=.1){
   const current=captionCorner(200,height);
   expect(current.curvature.every(value=>Number.isFinite(value)&&value>=1&&value<=Math.SQRT2)).toBe(true);
   expect(Math.max(...current.curvature.map((value,i)=>Math.abs(value-previous.curvature[i])))).toBeLessThan(.01);
   previous=current;
 }
});
it('rejects invalid geometry before allocating renderer data',()=>{
 for(const dimensions of [[-1,10,12],[10,Infinity,12],[10,10,NaN]])
  expect(()=>captionCorner(...dimensions as [number,number,number])).toThrow('Invalid caption corner');
});
