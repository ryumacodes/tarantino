import {expect,it} from 'vitest';
import {useEditorStore} from '../../stores/editor';
import {shortcutIntervals} from './intervals';
import {animatedShortcutPlan,keycapDraw,type KeycapAtlasEntry} from './animatedExport';
import {createShortcutMotion} from './motionTimeline';
const entry:KeycapAtlasEntry={tile:0,width:80,height:80,contentX:20,contentY:20,contentWidth:40,contentHeight:40};
it('export spans reproduce every visible preview sample across edits, rates and gaps',async()=>{
 for(const fps of [24,60,120]) {
  const state={...useEditorStore.getState(),videoFilePath:'/test.mp4',duration:6000,trimStart:0,trimEnd:6000,
    shortcutIntervals:shortcutIntervals([{timestamp:500,key:'s',modifiers:['command'],type:'down'}]),
    tracks:[{id:'v',name:'video',type:'video' as const,clips:[],height:60,order:0,visible:true,solo:false,muted:false,locked:false}],
    clips:[{id:'a',name:'fast',type:'video' as const,trackId:'v',startTime:0,sourceIn:0,sourceOut:2000,duration:1000,playbackRate:2,enabled:true,locked:false},
      {id:'b',name:'repeat',type:'video' as const,trackId:'v',startTime:3000,sourceIn:0,sourceOut:1000,duration:2000,playbackRate:.5,enabled:true,locked:false}]};
  const plan=await animatedShortcutPlan(state,640,416,fps,()=>entry,async()=>{});
  const preview=createShortcutMotion(state,640,416,fps);
  for(let frame=0;frame<plan.frames;frame++) {
    const expected=preview.sample(frame*1000/fps).filter(cap=>cap.opacity>0).map(cap=>keycapDraw(cap,entry));
    const span=plan.motion.find(s=>s.start_frame<=frame&&frame<s.end_frame);
    expect(span?.draws??[],`frame ${frame} at ${fps}fps`).toEqual(expected);
  }
 }
});
it('coalesces settled placements and skips empty multi-hour ranges',async()=>{
 const state={...useEditorStore.getState(),duration:36_000_000,trimStart:0,trimEnd:36_000_000,clips:[],tracks:[],
  shortcutIntervals:shortcutIntervals([{timestamp:500,key:'s',modifiers:['command'],type:'down'}])};
 let yields=0;
 const plan=await animatedShortcutPlan(state,640,416,120,()=>entry,async()=>{yields++;});
 expect(plan.frames).toBe(4_320_000);expect(yields).toBeLessThan(3);
 expect(plan.motion.some(span=>span.end_frame-span.start_frame>20)).toBe(true);
 expect(plan.motion.length).toBeLessThan(360);
});
