import {expect,it} from 'vitest';
import {captionBox} from './placement';
import {captionGroups} from './groups';
import type {CaptionMotionOptions} from './targets';
const options:CaptionMotionOptions={wordsReveal:'line-by-line',lineGrowth:'entire-line',wordEntrance:'appear'};
const group=captionGroups([{text:'one',startMs:100,endMs:200},{text:'two',startMs:300,endMs:500}],t=>({width:t.length*10,height:20}),500)[0];
it('fits the box inside short-side-relative padding at the chosen anchor',()=>{
 const stage={x:10,y:20,width:1200,height:600};
 expect(captionBox(group,150,options,stage,{x:0,y:0})).toEqual({x:60,y:70,width:70,height:20});
 expect(captionBox(group,150,options,stage,{x:1,y:1})).toEqual({x:1090,y:550,width:70,height:20});
});
it('anchors expansion to the complete line and recenters centered growth',()=>{
 const stage={x:0,y:0,width:1200,height:600},position={x:.5,y:.9};
 const full=captionBox(group,150,options,stage,position);
 const expanding=captionBox(group,150,{...options,lineGrowth:'left-anchored'},stage,position);
 const centered=captionBox(group,150,{...options,lineGrowth:'centered'},stage,position);
 expect(expanding.x).toBe(full.x);expect(expanding.width).toBe(30);
 expect(centered.x).toBe(full.x+20);
});
it('uses a bounded placeholder without a group and rejects invalid stages',()=>{
 const box=captionBox(null,0,options,{x:0,y:0,width:30,height:30},{x:.5,y:.5});
 expect(box.height).toBe(25);expect(box.width).toBe(.5);
 expect(()=>captionBox(null,0,options,{x:0,y:0,width:0,height:30},{x:.5,y:.5})).toThrow();
});
