import {expect,it} from 'vitest';
import {draggedMaskBounds} from './drag';
const rect={x:100,y:50,width:200,height:80};
it('draws in any direction and clamps to source edges with one-pixel minimum',()=>{
 expect(draggedMaskBounds(rect,{x:300,y:200},{x:50,y:20},'draw',800,400)).toEqual({x:50,y:20,width:250,height:180});
 expect(draggedMaskBounds(rect,{x:-20,y:-30},{x:900,y:500},'draw',800,400)).toEqual({x:0,y:0,width:800,height:400});
 expect(draggedMaskBounds(rect,{x:800,y:400},{x:800,y:400},'draw',800,400)).toEqual({x:799,y:399,width:1,height:1});
});
it('moves without accumulating deltas or changing size',()=>{
 expect(draggedMaskBounds(rect,{x:120,y:70},{x:150,y:100},'move',800,400)).toEqual({x:130,y:80,width:200,height:80});
 expect(draggedMaskBounds(rect,{x:120,y:70},{x:1000,y:900},'move',800,400)).toEqual({x:600,y:320,width:200,height:80});
});
it('resizes every corner, including crossing the fixed opposite corner',()=>{
 for(const [mode,point,expected] of [
  ['nw',{x:50,y:20},{x:50,y:20,width:250,height:110}],
  ['ne',{x:400,y:20},{x:100,y:20,width:300,height:110}],
  ['sw',{x:50,y:200},{x:50,y:50,width:250,height:150}],
  ['se',{x:400,y:200},{x:100,y:50,width:300,height:150}],
  ['nw',{x:400,y:200},{x:300,y:130,width:100,height:70}],
 ] as const)expect(draggedMaskBounds(rect,{x:0,y:0},point,mode,800,400)).toEqual(expected);
 expect(()=>draggedMaskBounds(rect,{x:0,y:0},{x:NaN,y:0},'draw',800,400)).toThrow();
});
