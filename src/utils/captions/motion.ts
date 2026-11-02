import {advanceCaptionSpring,createCaptionSpring,type CaptionSpring} from './spring';
import type {CaptionPartTarget} from './sceneTargets';
const fields=['x','y','opacity'] as const;
type Springs=Record<typeof fields[number],CaptionSpring>;
export interface CaptionFrameTargets {
  sourceKey:string;
  background:CaptionPartTarget;
  words:{groupIndex:number;wordIndex:number;target:CaptionPartTarget}[];
}
export interface CaptionFramePart extends CaptionPartTarget {blurRadius:number}
interface Part {key:string;target:CaptionPartTarget}
const parts=(frame:CaptionFrameTargets):Part[]=>[
  {key:JSON.stringify([frame.sourceKey,'background']),target:frame.background},
  ...frame.words.map(word=>({key:JSON.stringify([frame.sourceKey,word.groupIndex,word.wordIndex]),target:word.target})),
];
const rendered=(target:CaptionPartTarget):CaptionFramePart=>({...target,blurRadius:10*(1-Math.max(0,Math.min(1,target.opacity)))});
/** Recreate for transcript, layout or mapping edits. Frame targets are sampled
 * in output time; their provider resolves source time and nearby word groups. */
export class CaptionMotion {
  private frame=-1;
  private states=new Map<string,Springs>();
  private cache=new Map<number,Map<string,CaptionPartTarget>>();
  constructor(private fps:number,private targetAt:(outputMs:number)=>CaptionFrameTargets|null) {
    if(!Number.isFinite(fps)||fps<=0||fps>240)throw new RangeError('Invalid caption frame rate');
  }
  private step(frame:number,elapsed:number,reset=false) {
    const target=this.targetAt(frame*1000/this.fps),output=new Map<string,CaptionPartTarget>();
    const next=new Map<string,Springs>();
    for(const part of target?parts(target):[]) {
      const previous=reset?undefined:this.states.get(part.key);
      const state={} as Springs,cap={...part.target};
      for(const field of fields) {
        state[field]=previous?advanceCaptionSpring(previous[field],cap[field],elapsed):createCaptionSpring(cap[field]);
        cap[field]=state[field].value;
      }
      next.set(part.key,state);output.set(part.key,cap);
    }
    this.states=next;this.frame=frame;this.cache.set(frame,output);
    while(this.cache.size>Math.ceil(this.fps*2)+2)this.cache.delete(this.cache.keys().next().value!);
    return output;
  }
  private atFrame(frame:number) {
    const cached=this.cache.get(frame);if(cached)return cached;
    if(this.frame<0||frame<this.frame||frame-this.frame>this.fps) {
      this.cache.clear();this.states.clear();
      this.step(Math.max(0,Math.floor(frame-this.fps)),0,true);
    }
    while(this.frame<frame) {
      const next=this.frame+1;
      this.step(next,next*1000/this.fps-this.frame*1000/this.fps);
    }
    return this.cache.get(frame)!;
  }
  sample(outputMs:number):{background:CaptionFramePart;words:{groupIndex:number;wordIndex:number;target:CaptionFramePart}[]}|null {
    if(!Number.isFinite(outputMs)||outputMs<0)return null;
    const position=outputMs*this.fps/1000,frame=Math.floor(position),fraction=position-frame;
    const before=this.atFrame(frame),after=fraction?this.atFrame(frame+1):before;
    const target=this.targetAt(outputMs);if(!target)return null;
    const animated=parts(target).map(part=>{
      const a=before.get(part.key)??part.target,b=after.get(part.key)??a,cap={...part.target};
      for(const field of fields)cap[field]=a[field]+(b[field]-a[field])*fraction;
      return rendered(cap);
    });
    return {background:animated[0],words:target.words.map((word,index)=>({...word,target:animated[index+1]}))};
  }
}
