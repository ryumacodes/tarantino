import {expect,it} from 'vitest';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
import {previewSample} from '../timeline/previewTimeline';
import {maskOccurrences} from './timeline';
import type {RecordingMask} from './document';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const clip=(id:string,startTime:number,sourceIn:number,duration:number,playbackRate=1,sourceFilePath='a'):TimelineClip=>({
 id,name:id,type:'video',trackId:'v',startTime,sourceIn,duration,playbackRate,sourceFilePath,
 sourceOut:sourceIn+duration*playbackRate,enabled:true,locked:false,
});
const mask:RecordingMask={id:'private',sourceFilePath:'a',sourceStartMs:100,sourceEndMs:600,
 disabled:false,type:'sensitive-data',bounds:[{x:10,y:20,width:100,height:50}],highlightOpacity:.5};
it('projects coverage across trim, fast and slow repeats, and gaps',()=>{
 const state={videoFilePath:'a',duration:2000,tracks:[track],clips:[clip('fast',0,0,600,2),clip('repeat',1000,0,2400,.5)],trimStart:200,trimEnd:2500};
 const result=maskOccurrences(state,[mask]);
 expect(result.map(x=>[x.clipId,x.startMs,x.endMs,x.sourceStartMs,x.sourceEndMs])).toEqual([
  ['fast',0,100,400,600],['repeat',1000,2000,100,600],
 ]);
 expect(result[0].mask).toBe(mask);expect(mask.sourceStartMs).toBe(100);
});
it('matches canonical source coverage across reordering, covering tracks and disabled masks',()=>{
 const upper={...track,id:'upper',order:-1};
 const state={videoFilePath:'a',duration:2000,tracks:[track,upper],clips:[clip('base',0,0,1400),
  {...clip('cover',200,0,200,1,'b'),trackId:'upper'},clip('again',1500,0,400,2)],trimStart:50,trimEnd:1900};
 const masks=[mask,{...mask,id:'other',sourceFilePath:'b',sourceStartMs:0,sourceEndMs:150},
  {...mask,id:'disabled',disabled:true}];
 const result=maskOccurrences(state,masks);
 for(let time=0;time<1850;time+=5){
  const sample=previewSample(state,time+state.trimStart);
  const expected=sample.sourceTime===null?[]:masks.filter(m=>!m.disabled&&m.sourceFilePath===sample.sourceFilePath
   &&m.sourceStartMs<=sample.sourceTime!&&sample.sourceTime!<m.sourceEndMs).map(m=>m.id);
  expect(result.filter(x=>x.startMs<=time&&time<x.endMs).map(x=>x.mask.id)).toEqual(expected);
 }
 expect(maskOccurrences({...state,tracks:[{...track,visible:false},{...upper,visible:false}]},masks)).toEqual([]);
});
it('resolves legacy primary media and excludes unrelated sources and empty trims',()=>{
 const state={videoFilePath:'a',duration:1000,tracks:[],clips:[],trimStart:200,trimEnd:800};
 expect(maskOccurrences(state,[mask]).map(x=>[x.startMs,x.endMs])).toEqual([[0,400]]);
 expect(maskOccurrences({...state,videoFilePath:'b'},[mask])).toEqual([]);
 expect(maskOccurrences({...state,trimEnd:200},[mask])).toEqual([]);
});
