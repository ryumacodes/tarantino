import { expect, it } from 'vitest';
import { sampleWebcam } from './webcamTimeline';
import { DEFAULT_VISUAL_SETTINGS } from '../../stores/editor/constants';
import type { WebcamKeyframe } from '../../stores/editor/types';
const keys:WebcamKeyframe[]=[
  {id:'hide',time:1000,x:0.2,y:0.3,size:0.2,shape:'circle',visible:false},
  {id:'full',time:1500,x:0.2,y:0.3,size:0.2,shape:'roundrect',visible:true,layout:'fullscreen'},
];
it('matches native camera states on backward seeks, boundaries and legacy defaults',()=>{
  for (const time of [1600,200,1200,1500,999]) {
    const frame=sampleWebcam(DEFAULT_VISUAL_SETTINGS,keys,time);
    if(time<1000) expect(frame).toMatchObject({layout:'overlay',visible:true,size:0.15});
    else if(time<1500) expect(frame.visible).toBe(false);
    else expect(frame).toMatchObject({layout:'fullscreen',visible:true,x:0.5,y:0.5,size:1});
  }
});
it('honors hidden defaults and stable last-key wins for duplicate timestamps',()=>{
  expect(sampleWebcam({...DEFAULT_VISUAL_SETTINGS,webcamLayout:'hidden'},[],0).visible).toBe(false);
  expect(sampleWebcam(DEFAULT_VISUAL_SETTINGS,[keys[1],{...keys[0],time:1500}],1500).visible).toBe(false);
});

it('globally hides overlay and fullscreen states without changing the layout schedule',()=>{
  const visual={...DEFAULT_VISUAL_SETTINGS,webcamEnabled:false};
  const before=structuredClone(keys);
  for(const time of [1600,0,1000,1500,999]) expect(sampleWebcam(visual,keys,time).visible).toBe(false);
  expect(sampleWebcam({...visual,webcamEnabled:true},keys,1600)).toMatchObject({visible:true,layout:'fullscreen'});
  expect(keys).toEqual(before);
});
it('matches linear last-key selection for unsorted duplicates and reverse seeks without sorting the document',()=>{
 const schedule:WebcamKeyframe[]=Array.from({length:2000},(_,i)=>({id:String(i),time:((i*137)%997)*10,x:0.5,y:0.5,size:0.2,shape:'circle',visible:true}));
 const before=structuredClone(schedule);
 for(let i=1000;i>=0;i--){
  const time=i*10-1;
  let expected:WebcamKeyframe|undefined;
  for(const key of schedule)if(key.time<=time && (!expected || key.time>=expected.time))expected=key;
  expect(sampleWebcam(DEFAULT_VISUAL_SETTINGS,schedule,time).keyframeId).toBe(expected?.id);
 }
 expect(schedule).toEqual(before);
 const edited=schedule.map(k=>k.id==='1999'?{...k,time:11000}:k);
 expect(sampleWebcam(DEFAULT_VISUAL_SETTINGS,edited,11000).keyframeId).toBe('1999');
 expect(sampleWebcam(DEFAULT_VISUAL_SETTINGS,schedule,11000).keyframeId).not.toBe('1999');
});
