import {describe,it,expect} from 'vitest';
import {lensCameraCoverage} from './lensOverlap';
describe('lens camera coverage',()=>{
 it('distinguishes circle coverage from bounding-box overlap',()=>{
  expect(lensCameraCoverage(0,0,1,{x:.8,y:.8,width:.2,height:.2})).toBe(0);
  expect(lensCameraCoverage(0,0,1,{x:-.1,y:-.1,width:.2,height:.2})).toBeCloseTo(1,12);
  expect(lensCameraCoverage(0,0,1,{x:-1,y:-1,width:2,height:2})).toBeCloseTo(Math.PI/4,12);
  expect(lensCameraCoverage(0,0,1,{x:0,y:0,width:1,height:1})).toBeCloseTo(Math.PI/4,12);
 });
 it('is translation and scale invariant on partial intersections',()=>{
  const r={x:-.7,y:.3,width:1.2,height:.8};
  const expected=lensCameraCoverage(0,0,1,r);
  for(const scale of [.01,1,3840])expect(lensCameraCoverage(23,-19,scale,
   {x:23+r.x*scale,y:-19+r.y*scale,width:r.width*scale,height:r.height*scale})).toBeCloseTo(expected,10);
 });
 it('agrees with independent area sampling around the half-coverage boundary',()=>{
  for(const x of [-1.2,-.9,-.5,0,.3,.7]){
   const r={x,y:-.4,width:.8,height:.8};let inside=0;const n=500;
   for(let j=0;j<n;j++)for(let i=0;i<n;i++){
    const px=r.x+(i+.5)*r.width/n,py=r.y+(j+.5)*r.height/n;
    if(px*px+py*py<=1)inside++;
   }
   expect(Math.abs(lensCameraCoverage(0,0,1,r)-inside/(n*n))).toBeLessThan(.0002);
  }
 });
 it('returns zero for absent or invalid geometry',()=>{
  for(const radius of [0,-1,NaN,Infinity])expect(lensCameraCoverage(0,0,radius,{x:0,y:0,width:1,height:1})).toBe(0);
  expect(lensCameraCoverage(0,0,1,{x:0,y:0,width:0,height:1})).toBe(0);
 });
});
