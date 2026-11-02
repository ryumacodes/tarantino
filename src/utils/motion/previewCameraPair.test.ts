import {it,expect} from 'vitest';
import {previewCameraPair} from './previewCameraPair';
import type {CameraFrame} from './zoomTrajectory';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,order:0,visible:true,solo:false,muted:false,locked:false};
const clip=(id:string,startTime:number,sourceIn:number,rate:number):TimelineClip=>({id,name:id,type:'video',trackId:'v',startTime,sourceIn,duration:1000,sourceOut:sourceIn+1000*rate,playbackRate:rate,enabled:true,locked:false});
const frames:CameraFrame[]=Array.from({length:800},(_,i)=>[1+i*i/10000,.5+i/10000,.5]);
const fast={duration:2000,tracks:[track],clips:[clip('fast',250,1000,2)]};
it('samples nonlinear motion at adjacent output frames without multiplying clip speed twice',()=>{
 for(const [fps,index] of [[24,170],[30,172],[60,176],[120,178]]){
  const pair=previewCameraPair(fast,500,frames,fps);
  expect(pair.now).toEqual(frames[180]);expect(pair.before).toEqual(frames[index]);expect(pair.canBlur).toBe(true);
 }
 const slow={...fast,clips:[clip('slow',250,1000,.5)]};
 const pair=previewCameraPair(slow,500,frames,60);
 expect(pair.now).toEqual(frames[135]);expect(pair.before).toEqual(frames[134]);
});
it('holds the current pose throughout the first output interval after a cut or gap',()=>{
 const state={...fast,clips:[clip('old',0,0,1),clip('new',1000,4000,2)]};
 for(const fps of [24,30,60,120]){
  for(const t of [1000,1000+1000/fps*.25,1000+1000/fps*.9]){
   const pair=previewCameraPair(state,t,frames,fps);
   expect(pair.canBlur).toBe(false);expect(pair.before).toEqual(pair.now);
  }
  expect(previewCameraPair(state,1000+1000/fps+1,frames,fps).canBlur).toBe(true);
  const gap={...state,clips:[state.clips[1]]};
  expect(previewCameraPair(gap,1001,frames,fps).canBlur).toBe(false);
 }
});
it('does not sample trimmed-out source motion at the start of a legacy project',()=>{
 const legacy={duration:2000,tracks:[],clips:[]};
 expect(previewCameraPair(legacy,0,frames,60).canBlur).toBe(false);
 expect(previewCameraPair(legacy,10,frames,60).canBlur).toBe(false);
 expect(previewCameraPair(legacy,20,frames,60).canBlur).toBe(true);
});
