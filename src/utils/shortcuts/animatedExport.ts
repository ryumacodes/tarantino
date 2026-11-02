import type {EditorState} from '../../stores/editor/types';
import type {AnimatedKeycap} from './keycapMotion';
import {createShortcutMotion} from './motionTimeline';
import {shortcutExportPlan} from './exportPlan';

export interface KeycapAtlasEntry {tile:number;width:number;height:number;contentX:number;contentY:number;contentWidth:number;contentHeight:number}
export interface KeycapDraw {tile:number;x:number;y:number;width:number;height:number;opacity:number}
export interface KeycapMotionSpan {start_frame:number;end_frame:number;draws:KeycapDraw[]}
export function keycapDraw(cap:AnimatedKeycap,entry:KeycapAtlasEntry):KeycapDraw {
  const sx=cap.width/entry.contentWidth,sy=cap.height/entry.contentHeight;
  return {tile:entry.tile,x:cap.x-entry.contentX*sx,y:cap.y-entry.contentY*sy,
    width:entry.width*sx,height:entry.height*sy,opacity:Math.max(0,Math.min(1,cap.opacity))};
}

/** Reuse atlas pixels; only compact placement instructions vary per frame. */
export async function animatedShortcutPlan(state:EditorState,width:number,height:number,fps:number,
  atlas:(cap:AnimatedKeycap)=>KeycapAtlasEntry,yieldControl:()=>Promise<void>=()=>new Promise(resolve=>setTimeout(resolve,0))) {
  const plan=shortcutExportPlan(state,fps),motion:KeycapMotionSpan[]=[];
  if(!plan.spans.length)return {frames:plan.frames,motion};
  const sampler=createShortcutMotion(state,width,height,fps);
  // Only visit visible sections and their spring decay; skip long empty gaps.
  const windows:{start:number;end:number}[]=[];
  for(const span of plan.spans) {
    const start=Math.max(0,span.start_frame-1),end=Math.min(plan.frames,span.end_frame+Math.ceil(fps));
    const previous=windows[windows.length-1];
    if(previous&&start<=previous.end)previous.end=Math.max(previous.end,end);else windows.push({start,end});
  }
  let instructions=0,processed=0,previousKey='';
  for(const window of windows)for(let frame=window.start;frame<window.end;frame++) {
    const draws=sampler.sample(frame*1000/fps).filter(cap=>cap.opacity>0)
      .map(cap=>keycapDraw(cap,atlas(cap)));
    if(draws.length) {
      const key=JSON.stringify(draws),previous=motion[motion.length-1];
      if(previous&&previous.end_frame===frame&&key===previousKey)previous.end_frame=frame+1;
      else {
        instructions+=draws.length;
        if(instructions>1_000_000)throw new Error('Shortcut animation exceeds the export memory limit');
        motion.push({start_frame:frame,end_frame:frame+1,draws});previousKey=key;
      }
    }
    if(++processed%240===0)await yieldControl();
  }
  return {frames:plan.frames,motion};
}
