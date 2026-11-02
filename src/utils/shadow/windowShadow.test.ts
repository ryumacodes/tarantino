import {describe,it,expect} from 'vitest';
import {blurShadow,windowShadowPixels} from './windowShadow';

describe('window shadow',()=>{
 it('agrees with direct separable convolution including radii larger than the image',()=>{
  for(const [w,h] of [[1,17],[19,1],[9,7]])for(const radius of [0,1,3,20,200]) {
   const source=Float32Array.from({length:w*h},(_,i)=>(i*73%256)/255),actual=source.slice();
   const tmp=new Float32Array(source.length),expected=tmp.slice();
   for(let y=0;y<h;y++)for(let x=0;x<w;x++) {
    let sum=0;for(let d=-radius;d<=radius;d++)sum+=source[y*w+Math.max(0,Math.min(w-1,x+d))];
    tmp[y*w+x]=sum/(2*radius+1);
   }
   for(let y=0;y<h;y++)for(let x=0;x<w;x++) {
    let sum=0;for(let d=-radius;d<=radius;d++)sum+=tmp[Math.max(0,Math.min(h-1,y+d))*w+x];
    expected[y*w+x]=sum/(2*radius+1);
   }
   blurShadow(actual,w,h,radius);
   actual.forEach((value,i)=>expect(value).toBeCloseTo(expected[i],6));
  }
 });
 it('places signed offsets in output pixels and preserves transparent exterior',()=>{
  const pixels=windowShadowPixels({width:9,height:7,rect:{x:2,y:3,width:3,height:2},radius:0,blur:0,intensity:.5});
  for(let y=0;y<7;y++)for(let x=0;x<9;x++)expect(pixels[(y*9+x)*4+3]).toBe(x>=2&&x<5&&y>=3&&y<5?127:0);
 });
});
