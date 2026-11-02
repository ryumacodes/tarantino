import type {CaptionGroup} from './groups';
import type {TimedTranscriptText} from './timing';
import {captionTargets,type CaptionMotionOptions} from './targets';
export interface CaptionRect {x:number;y:number;width:number;height:number}
export interface CaptionPosition {x:number;y:number}
function fit(width:number,height:number,bounds:CaptionRect,position:CaptionPosition):CaptionRect {
  const scale=Math.min(1,bounds.width/width,bounds.height/height);
  const w=width*scale,h=height*scale;
  return {x:bounds.x+(bounds.width-w)*position.x,y:bounds.y+(bounds.height-h)*position.y,width:w,height:h};
}
/** Unsprung stage-space box. Local word targets remain separate for animation. */
export function captionBox<T extends TimedTranscriptText>(group:CaptionGroup<T>|null,time:number,
  options:CaptionMotionOptions,stage:CaptionRect,position:CaptionPosition):CaptionRect {
  if(![stage.x,stage.y,stage.width,stage.height,position.x,position.y,time].every(Number.isFinite)
    ||stage.width<=0||stage.height<=0)throw new RangeError('Invalid caption placement');
  const side=Math.min(stage.width,stage.height),inset=(side-side/1.2)/2;
  const bounds={x:stage.x+inset,y:stage.y+inset,width:stage.width-2*inset,height:stage.height-2*inset};
  if(!group)return fit(1,50,bounds,position);
  const target=captionTargets(group,time,options);
  if(options.wordsReveal==='line-by-line'&&options.wordEntrance!=='fade-in'&&options.lineGrowth==='left-anchored') {
    const full=fit(group.width,group.height,bounds,position);
    return {...full,width:target.width};
  }
  return fit(target.width,target.height,bounds,position);
}
