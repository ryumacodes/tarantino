import {expect,it} from 'vitest';
import {captionGroups} from './groups';
import {captionGroupIndex,captionSceneTargets} from './sceneTargets';
import {captionBox} from './placement';
import type {CaptionMotionOptions} from './targets';
const groups=captionGroups([{text:'one',startMs:100,endMs:200},{text:'a longer line',startMs:2000,endMs:2200}],t=>({width:t.length*10,height:20}),500);
const options:CaptionMotionOptions={wordsReveal:'line-by-line',lineGrowth:'entire-line',wordEntrance:'appear'};
const stage={x:0,y:0,width:1200,height:600},position={x:.5,y:.9};
it('retains the last group during silence and switches at the next start',()=>{
 expect(captionGroupIndex(groups,0)).toBe(0);expect(captionGroupIndex(groups,1500)).toBe(0);
 expect(captionGroupIndex(groups,2000)).toBe(1);
});
it('moves disappearing words with the current box and pads the background by font size',()=>{
 const scene=captionSceneTargets(groups,2050,options,stage,position,40);
 const anchor=captionBox(groups[1],2050,options,stage,position);
 expect(scene.words[0].target.x).toBe(anchor.x);expect(scene.words[0].target.opacity).toBe(0);
 expect(scene.words[1].target.opacity).toBe(1);
 expect(scene.background).toEqual({...anchor,x:anchor.x-20,y:anchor.y-10,width:anchor.width+40,height:anchor.height+20,opacity:1});
});
it('allows rendering only selected word groups without changing their shared anchor',()=>{
 const all=captionSceneTargets(groups,2050,options,stage,position,40);
 const selected=captionSceneTargets(groups,2050,options,stage,position,40,[0]);
 expect(selected.background).toEqual(all.background);expect(selected.words).toEqual([all.words[0]]);
});
