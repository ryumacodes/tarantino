import {expect,it} from 'vitest';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
import {captionOccurrences,TranscriptIndex} from './timeline';
import {previewSample} from '../timeline/previewTimeline';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const clip=(id:string,startTime:number,sourceIn:number,duration:number,playbackRate=1,sourceFilePath='a'):TimelineClip=>({
 id,name:id,type:'video',trackId:'v',startTime,sourceIn,duration,playbackRate,sourceFilePath,
 sourceOut:sourceIn+duration*playbackRate,enabled:true,locked:false,
});
const words=[{text:'first',startMs:100,endMs:600},{text:'second',startMs:700,endMs:1100}];
it('maps trims, speed, gaps and repeated source words without modifying them',()=>{
 const state={duration:2000,tracks:[track],clips:[clip('fast',0,0,600,2),clip('repeat',1000,0,2400,.5)],trimStart:200,trimEnd:2500};
 const result=captionOccurrences(state,()=>new TranscriptIndex(words));
 expect(result.map(x=>[x.clipId,x.wordIndex,x.startMs,x.endMs])).toEqual([
  ['fast',0,0,100],['fast',1,150,350],['repeat',0,1000,2000],['repeat',1,2200,2300],
 ]);
 expect(result[0].sourceStartMs).toBe(400);expect(words[0].startMs).toBe(100);
});
it('isolates sources and follows hidden, solo and covering track policy',()=>{
 const upper={...track,id:'upper',order:-1};
 const state={duration:1200,tracks:[track,upper],clips:[clip('base',0,0,1200),{...clip('cover',200,0,200,1,'b'),trackId:'upper'}],trimStart:0,trimEnd:1200};
 const source=(path:string|undefined)=>path==='a'?new TranscriptIndex(words):undefined;
 expect(captionOccurrences(state,source).map(x=>[x.startMs,x.endMs])).toEqual([[100,200],[400,600],[700,1100]]);
 expect(captionOccurrences({...state,tracks:[track,{...upper,solo:true}]},source)).toEqual([]);
 expect(captionOccurrences({...state,tracks:[{...track,visible:false},upper]},source)).toEqual([]);
});
it('queries overlapping unsorted words with half-open boundaries and stable original indexes',()=>{
 const index=new TranscriptIndex([{text:'late',startMs:20,endMs:30},{text:'long',startMs:0,endMs:100},
  {text:'empty',startMs:25,endMs:25}]);
 expect(index.intersect(25,30).map(x=>x.index)).toEqual([1,0]);
 expect(index.intersect(30,31).map(x=>x.index)).toEqual([1]);
 expect(index.intersect(100,110)).toEqual([]);
});
it('agrees with canonical playback sampling across reordered multi-source clips',()=>{
 const sources={a:new TranscriptIndex(words),b:new TranscriptIndex([{text:'other',startMs:0,endMs:400}])};
 const state={duration:2000,tracks:[track],clips:[clip('one',0,600,400,2),clip('two',500,0,800,.5,'b'),clip('three',1400,0,600)],trimStart:50,trimEnd:1900};
 const projected=captionOccurrences(state,path=>sources[path as keyof typeof sources]);
 for(let time=0;time<1850;time+=7){
  const sample=previewSample(state,time+state.trimStart),source=sources[sample.sourceFilePath as keyof typeof sources];
  const expected=sample.sourceTime===null?[]:source?.intersect(sample.sourceTime,sample.sourceTime+.000001).map(x=>x.word.text)??[];
  expect(projected.filter(x=>x.startMs<=time&&time<x.endMs).map(x=>x.word.text)).toEqual(expected);
 }
});
it('supports legacy recordings and empty or invalid ranges',()=>{
 const state={duration:1200,tracks:[],clips:[],trimStart:200,trimEnd:800};
 expect(captionOccurrences(state,()=>new TranscriptIndex(words)).map(x=>[x.startMs,x.endMs])).toEqual([[0,400],[500,600]]);
 expect(captionOccurrences({...state,trimEnd:100},()=>new TranscriptIndex(words))).toEqual([]);
 expect(()=>new TranscriptIndex([{text:'bad',startMs:20,endMs:10}])).toThrow();
});
