import {expect,it} from 'vitest';
import {keycapMipmaps,bottomUpPixels} from './keycapMipmaps';
it('removes transparent RGB before filtering and rounds each RGBA8 level',()=>{
 const levels=keycapMipmaps(2,2,new Uint8Array([255,0,0,255,0,255,0,0,0,255,0,0,0,255,0,0]));
 expect([...levels[1].pixels]).toEqual([64,0,0,64]);
 expect([...levels[0].pixels.slice(4,8)]).toEqual([0,0,0,0]);
});
it('uses binomial weights after the first level instead of repeated box filtering',()=>{
 const rgba=new Uint8Array(8*4);for(let x=0;x<8;x++)rgba[x*4+3]=x>=2&&x<4?255:0;
 const levels=keycapMipmaps(8,1,rgba);
 expect(levels.map(l=>[l.width,l.height])).toEqual([[8,1],[4,1],[2,1],[1,1]]);
 expect([...levels[2].pixels].filter((_,i)=>i%4===3)).toEqual([96,32]);
});
it('handles odd and narrow rasters with clamped borders and explicit upload orientation',()=>{
 const rgba=new Uint8Array(3*3*4);for(let i=0;i<9;i++){rgba[i*4]=i;rgba[i*4+3]=255;}
 const levels=keycapMipmaps(3,3,rgba);
 expect(levels.map(l=>[l.width,l.height])).toEqual([[3,3],[1,1]]);
 expect(levels[1].pixels[0]).toBe(2);
 expect(bottomUpPixels(levels[0])[0]).toBe(6);
 expect(bottomUpPixels(levels[0])[24]).toBe(0);
 expect(()=>keycapMipmaps(0,3,rgba)).toThrow(RangeError);
});
