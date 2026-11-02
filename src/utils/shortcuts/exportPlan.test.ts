import {expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from '../../stores/editor';
import {shortcutIntervals} from './intervals';
import {shortcutExportPlan} from './exportPlan';
import {shortcutAtOutputTime} from './timeline';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,order:0,visible:true,solo:false,muted:false,locked:false};
const clip=(id:string,startTime:number,sourceIn:number,duration:number,playbackRate:number):TimelineClip=>({
  id,name:id,type:'video',trackId:'v',startTime,sourceIn,sourceOut:sourceIn+duration*playbackRate,
  duration,playbackRate,enabled:true,locked:false,
});
it('matches every exported sample through trims, repeated clips, speeds, overlaps and hidden entries',()=>{
  const intervals=shortcutIntervals([100,500,1800,1900,3500,4500].map((timestamp,i)=>({timestamp,key:i%2?'c':'v',type:'down',modifiers:['command']})));
  for(const fps of [24,30,60,120])for(const rate of [0.5,1,1.25,3])for(const hidden of [[],[500,1900]]) {
    const state={...useEditorStore.getState(),videoFilePath:'/take.mp4',duration:6000,trimStart:103,trimEnd:4701,
      shortcutIntervals:intervals,shortcutSettings:{...useEditorStore.getState().shortcutSettings,hidden},
      tracks:[track],clips:[clip('first',0,0,1000,rate),clip('repeated',1300,0,2000,rate),clip('last',3300,3400,1500,1)]};
    const plan=shortcutExportPlan(state,fps);
    for(let frame=0;frame<plan.frames;frame++) {
      const expected=shortcutAtOutputTime(state,frame*1000/fps,()=>({intervals,hidden:new Set(hidden)}));
      const span=plan.spans.find(s=>s.start_frame<=frame&&frame<s.end_frame);
      const actual=span?plan.labels[span.tile]:null;
      expect(actual?.key??null).toBe(expected?.key??null);
      expect(actual?.modifiers??null).toEqual(expected?.modifiers??null);
    }
    expect(plan.labels.length).toBeLessThanOrEqual(2);
  }
});
it('does no per-frame expansion for a long recording and emits nothing when disabled or unavailable',()=>{
  const state={...useEditorStore.getState(),duration:36_000_000,trimStart:0,trimEnd:36_000_000,tracks:[],clips:[],
    shortcutIntervals:shortcutIntervals([{timestamp:1000,key:'c',type:'down',modifiers:['command']}])};
  const plan=shortcutExportPlan(state,120);
  expect(plan.frames).toBe(4_320_000);expect(plan.spans).toHaveLength(1);expect(plan.labels).toHaveLength(1);
  expect(shortcutExportPlan({...state,shortcutSettings:{...state.shortcutSettings,enabled:false}},120).spans).toEqual([]);
  expect(shortcutExportPlan({...state,shortcutIntervals:[]},120).spans).toEqual([]);
});
it('adds camera layout boundaries even when the shortcut remains otherwise unchanged',()=>{
  const state={...useEditorStore.getState(),duration:3000,trimStart:0,trimEnd:3000,tracks:[],clips:[],hasWebcam:true,
    shortcutIntervals:shortcutIntervals([{timestamp:1100,key:'c',type:'down',modifiers:['command']}]),
    visualSettings:{...useEditorStore.getState().visualSettings,webcamEnabled:true},
    webcamKeyframes:[{id:'full',time:1500,x:0.5,y:0.5,size:0.2,shape:'circle' as const,visible:true,layout:'fullscreen' as const},
      {id:'overlay',time:2000,x:0.5,y:0.5,size:0.2,shape:'circle' as const,visible:true,layout:'overlay' as const}]};
  expect(shortcutExportPlan(state,60).spans).toEqual([{start_frame:60,end_frame:90,tile:0},{start_frame:120,end_frame:157,tile:0}]);
});
