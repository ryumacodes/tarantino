import {expect,it} from 'vitest';
import {captionGroups} from './groups';
import {captionTargets,type CaptionMotionOptions} from './targets';
const group=captionGroups([{text:'one',startMs:100,endMs:200},{text:'two',startMs:400,endMs:500}],t=>({width:t.length*10,height:20}),500)[0];
const options:CaptionMotionOptions={wordsReveal:'line-by-line',lineGrowth:'centered',wordEntrance:'appear'};
it('dims upcoming words for fade-in and reserves full line geometry',()=>{
 const target=captionTargets(group,150,{...options,wordEntrance:'fade-in'});
 expect(target.words.map(w=>w.opacity)).toEqual([1,.5]);expect(target.width).toBe(group.width);
});
it('retains full-line anchoring for expansion and shifts incoming words',()=>{
 const target=captionTargets(group,150,{...options,lineGrowth:'left-anchored',wordEntrance:'slide-in'});
 expect(target.width).toBe(30);expect(target.anchorWidth).toBe(group.width);
 expect(target.words[1].x).toBe(group.offsets[1]+40);
});
it('pushes completed words out and incoming words past the current word',()=>{
 const target=captionTargets(group,250,{...options,wordsReveal:'word-by-word'});
 expect(target.words.map(w=>[w.x,w.opacity])).toEqual([[-30,0],[30,0]]);
 expect(captionTargets(group,400,{...options,wordsReveal:'word-by-word'}).words[1].opacity).toBe(1);
});
it('uses inclusive source endpoints before hiding the completed group',()=>{
 expect(captionTargets(group,group.endMs,options).opacity).toBe(1);
 expect(captionTargets(group,group.endMs+.001,options).opacity).toBe(0);
 expect(captionTargets(group,99,options).presentWordCount).toBe(0);
});
