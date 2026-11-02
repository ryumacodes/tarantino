import type {TimedTranscriptText} from './timing';
import type {CaptionGroup} from './groups';
import {captionTargets,type CaptionMotionOptions} from './targets';
import {captionBox,type CaptionPosition,type CaptionRect} from './placement';
export interface CaptionPartTarget extends CaptionRect {opacity:number}
/** Active group selection retains the previous box through inter-group silence. */
export function captionGroupIndex<T>(groups:readonly CaptionGroup<T>[],time:number):number {
  let low=0,high=groups.length;
  while(low<high){const mid=(low+high)>>>1;if(groups[mid].startMs<=time)low=mid+1;else high=mid;}
  if(low===0)return 0;
  const index=low-1,current=groups[index],next=groups[index+1];
  if(current.endMs<time||!next)return index;
  return time-current.endMs<next.startMs-time?index:index+1;
}
/** Background and word targets share the selected group's stage anchor. Width
 * and height are immediate values; only x/y/opacity should pass through springs. */
export function captionSceneTargets<T extends TimedTranscriptText>(groups:readonly CaptionGroup<T>[],time:number,
  options:CaptionMotionOptions,stage:CaptionRect,position:CaptionPosition,fontSize:number,
  wordGroupIndexes:readonly number[]=groups.map((_,index)=>index)) {
  if(!Number.isFinite(fontSize)||fontSize<=0)throw new RangeError('Invalid caption font size');
  const groupIndex=captionGroupIndex(groups,time),group=groups[groupIndex]??null;
  const anchor=captionBox(group,time,options,stage,position);
  const opacity=group?captionTargets(group,time,options).opacity:0;
  const background:CaptionPartTarget={x:anchor.x-fontSize*.5,y:anchor.y-fontSize*.25,
    width:anchor.width+fontSize,height:anchor.height+fontSize*.5,opacity};
  const words:{groupIndex:number;wordIndex:number;target:CaptionPartTarget}[]=[];
  for(const index of wordGroupIndexes) {
    const wordGroup=groups[index];if(!wordGroup)continue;
    const targets=captionTargets(wordGroup,time,options);
    targets.words.forEach((word,wordIndex)=>words.push({groupIndex:index,wordIndex,
      target:{...word,x:anchor.x+word.x,y:anchor.y+word.y}}));
  }
  return {groupIndex,background,words};
}
