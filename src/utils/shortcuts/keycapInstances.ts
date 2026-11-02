import type {ShortcutInterval} from './intervals';
import {shortcutLabels} from './keycaps';
export interface KeycapSegment {interval:ShortcutInterval;index:number}
export interface KeycapInstance {
  label:string;
  segments:KeycapSegment[];
  start:number;
  end:number;
  appearTime:number;
  handoffStart?:number;
}

/** Shared visible keys continue across overlapping chords instead of reappearing. */
export function keycapInstances(intervals:readonly ShortcutInterval[],
  hidden:ReadonlySet<number>=new Set()):KeycapInstance[] {
  const instances:KeycapInstance[]=[],active=new Map<string,KeycapInstance>();
  let previous:ShortcutInterval|undefined;
  for(const interval of intervals) {
    if(hidden.has(interval.firstDown))continue;
    const labels=shortcutLabels(interval);
    if(!previous||interval.start>previous.end)active.clear();
    else for(const [label,instance] of active) {
      if(!labels.includes(label)) {
        instance.handoffStart=interval.start;
        instance.end=interval.start+100;
        active.delete(label);
      }
    }
    labels.forEach((label,index)=>{
      const existing=active.get(label);
      if(existing){existing.segments.push({interval,index});existing.end=interval.end;return;}
      const instance:KeycapInstance={label,segments:[{interval,index}],start:interval.start,
        end:interval.end,appearTime:interval.start+index*60};
      active.set(label,instance);instances.push(instance);
    });
    previous=interval;
  }
  return instances;
}
const clamp=(value:number)=>Math.max(0,Math.min(1,value));
export function keycapTarget(instance:KeycapInstance,sourceTime:number,fullscreenCamera=false) {
  let low=0,high=instance.segments.length;
  while(low<high) {
    const middle=(low+high)>>>1;
    if(instance.segments[middle].interval.start<=sourceTime)low=middle+1;else high=middle;
  }
  const segment=instance.segments[Math.max(0,low-1)];
  const visible=Number.isFinite(sourceTime)&&instance.start<=sourceTime&&sourceTime<=instance.end;
  const enter=clamp((sourceTime-instance.appearTime)/100);
  const exit=instance.handoffStart===undefined?0:clamp((sourceTime-instance.handoffStart)/100);
  return {segment,opacity:visible&&!fullscreenCamera?(exit>0?1-exit:enter):0,
    travel:visible?-1+(1-exit)*enter:-1};
}
