import {expect,it} from 'vitest';
import {captionGroups} from './groups';
const measure=(text:string)=>({width:text.length*10,height:20});
it('measures shared spaces and trims edges without altering transcript words',()=>{
 const words=[{text:'  one ',startMs:0,endMs:100},{text:' two  ',startMs:200,endMs:300}];
 const group=captionGroups(words,measure,500)[0];
 expect(group.words.map(w=>w.text)).toEqual(['one ',' two']);
 expect(group.offsets).toEqual([0,40,80]);expect(group.heights).toEqual([0,20,20]);
 expect(group.endMs).toBe(600);expect(words[1].endMs).toBe(300);
});
it('splits at source pauses and bounds the final hold by the next group',()=>{
 const groups=captionGroups([{text:'a',startMs:0,endMs:100},{text:'b',startMs:1000,endMs:1100},
  {text:'c',startMs:1200,endMs:1300}],measure,500);
 expect(groups.map(g=>g.words.map(w=>w.text))).toEqual([['a',' b'],['c']]);
 expect(groups.map(g=>g.endMs)).toEqual([1200,1600]);
});
it('rejects unusable renderer dimensions',()=>{
 expect(()=>captionGroups([],measure,NaN)).toThrow();
 expect(()=>captionGroups([],()=>({width:Infinity,height:20}),500)).toThrow();
});
