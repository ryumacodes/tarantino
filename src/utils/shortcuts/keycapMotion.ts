import type {KeycapInstance} from './keycapInstances';
import type {ShortcutInterval} from './intervals';
import {keycapTarget} from './keycapInstances';
import {shortcutLabels,shortcutLayout} from './keycaps';
import type {ShortcutSettings} from './settings';
import {advanceKeycapSpring,createKeycapSpring,type KeycapSpring} from './keycapSpring';

export interface AnimatedKeycap {label:string;x:number;y:number;width:number;height:number;opacity:number}
export interface KeycapSourceTime {time:number;fullscreenCamera?:boolean}
const fields=['x','y','width','height','opacity'] as const;
type MotionState=Record<typeof fields[number],KeycapSpring>;
type LayoutCache=ReadonlyMap<ShortcutInterval,ReturnType<typeof shortcutLayout>>;

/** Source targets are sampled on the output clock, so speed edits affect targets
 * while the spring's response duration remains in playback milliseconds. */
export function keycapMotionTarget(instance:KeycapInstance,source:KeycapSourceTime|null,
  settings:ShortcutSettings,width:number,height:number,layouts?:LayoutCache):AnimatedKeycap {
  const target=keycapTarget(instance,source?.time??0,source?.fullscreenCamera);
  return positionTarget(target,source!==null,settings,width,height,layouts);
}

function positionTarget(target:ReturnType<typeof keycapTarget>,hasSource:boolean,
  settings:ShortcutSettings,width:number,height:number,layouts?:LayoutCache):AnimatedKeycap {
  const caps=layouts?.get(target.segment.interval)??shortcutLayout(shortcutLabels(target.segment.interval),settings,width,height);
  const cap=caps[target.segment.index];
  return {...cap,y:cap.y+target.travel*cap.height*-0.033,opacity:hasSource?target.opacity:0};
}

/** Bounded seek reconstruction with a frame cache shared by forward playback.
 * Recreate when the edit mapping, dimensions, settings or source data change. */
export class KeycapMotion {
  private frame=-1;
  private lastTargets:{segment:ReturnType<typeof keycapTarget>["segment"];travel:number;opacity:number;cap:AnimatedKeycap}[]=[];
  private layouts=new Map<ShortcutInterval,ReturnType<typeof shortcutLayout>>();
  private states:MotionState[]=[];
  private cache=new Map<number,AnimatedKeycap[]>();
  constructor(private instances:readonly KeycapInstance[],private settings:ShortcutSettings,
    private width:number,private height:number,private fps:number,
    private sourceAt:(outputTime:number)=>KeycapSourceTime|null) {
    if(!Number.isFinite(fps)||fps<=0||fps>240)throw new RangeError('Invalid keycap frame rate');
    for(const instance of instances)for(const {interval} of instance.segments) {
      if(!this.layouts.has(interval))this.layouts.set(interval,shortcutLayout(shortcutLabels(interval),settings,width,height));
    }
  }
  private targets(frame:number) {
    const source=this.sourceAt(frame*1000/this.fps);
    return this.instances.map((instance,index)=>{
      const target=keycapTarget(instance,source?.time??0,source?.fullscreenCamera);
      const opacity=source?target.opacity:0,previous=this.lastTargets[index];
      if(previous&&previous.segment===target.segment&&previous.travel===target.travel&&previous.opacity===opacity)
        return previous.cap;
      const cap=positionTarget(target,source!==null,this.settings,this.width,this.height,this.layouts);
      this.lastTargets[index]={segment:target.segment,travel:target.travel,opacity,cap};
      return cap;
    });
  }
  private atFrame(frame:number):AnimatedKeycap[] {
    const cached=this.cache.get(frame);if(cached)return cached;
    if(this.frame<0||frame<this.frame||frame-this.frame>this.fps) {
      this.frame=Math.max(0,Math.floor(frame-this.fps));
      const initial=this.targets(this.frame);
      this.states=initial.map(cap=>Object.fromEntries(fields.map(field=>[field,createKeycapSpring(cap[field])])) as MotionState);
      this.cache.clear();this.cache.set(this.frame,initial);
    }
    while(this.frame<frame) {
      const next=this.frame+1,targets=this.targets(next);
      const elapsed=next*1000/this.fps-this.frame*1000/this.fps;
      const caps=targets.map((target,index)=>{
        const cap={...target};
        for(const field of fields) {
          const state=advanceKeycapSpring(this.states[index][field],target[field],elapsed);
          this.states[index][field]=state;cap[field]=state.value;
        }
        return cap;
      });
      this.frame=next;this.cache.set(next,caps);
      while(this.cache.size>Math.ceil(this.fps*2)+2)this.cache.delete(this.cache.keys().next().value!);
    }
    return this.cache.get(frame)!;
  }
  sample(outputTime:number):AnimatedKeycap[] {
    if(!Number.isFinite(outputTime)||outputTime<0)return [];
    const position=outputTime*this.fps/1000,frame=Math.floor(position),fraction=position-frame;
    const before=this.atFrame(frame);
    if(fraction===0)return before.map(cap=>({...cap}));
    const after=this.atFrame(frame+1);
    return before.map((cap,index)=>{
      const result={...cap};
      for(const field of fields)result[field]=cap[field]+(after[index][field]-cap[field])*fraction;
      return result;
    });
  }
}
