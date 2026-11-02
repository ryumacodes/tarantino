import type {TimedTranscriptText} from './timing';
import type {CaptionGroup} from './groups';
export interface CaptionMotionOptions {
  wordsReveal:'line-by-line'|'word-by-word';
  lineGrowth:'entire-line'|'left-anchored'|'centered';
  wordEntrance:'appear'|'fade-in'|'slide-in';
}
export interface CaptionWordTarget {x:number;y:number;width:number;height:number;opacity:number}
const phase=(time:number,start:number,end:number)=>time<start?-1:time>end?1:0;
/** Unsprung local-space targets. Animation integrates these on the output clock,
 * while visibility and word selection use the mapped source time. */
export function captionTargets<T extends TimedTranscriptText>(group:CaptionGroup<T>,time:number,options:CaptionMotionOptions) {
  const words=group.words;
  if(!words.length||!Number.isFinite(time))throw new RangeError('Invalid caption target input');
  let low=0,high=words.length;
  while(low<high){const mid=(low+high)>>>1;if(words[mid].startMs<=time)low=mid+1;else high=mid;}
  const count=low,current=words[Math.max(0,count-1)];
  const progress=phase(time,group.startMs,group.endMs);
  const growth=options.wordEntrance==='fade-in'?'entire-line':options.lineGrowth;
  let width=group.width,height=group.height;
  if(options.wordsReveal==='word-by-word'){width=current.width;height=current.height;}
  else if(growth!=='entire-line') {
    width=group.offsets[count];
    if(growth==='centered')height=count?group.heights[count]:words[0].height;
  }
  const targets:CaptionWordTarget[]=words.map((word,index)=>{
    const own=phase(time,word.startMs,word.endMs),wordPhase=progress===0?own:progress;
    let x=group.offsets[index],opacity=0;
    if(options.wordsReveal==='word-by-word') {
      x=progress===1||own===1?-word.width:own===0?0:current.width;
      opacity=progress===0&&wordPhase===0?1:0;
    }else {
      if(options.wordEntrance==='slide-in'&&wordPhase===-1)x+=40;
      if(progress===0)opacity=wordPhase===-1?(options.wordEntrance==='fade-in'?.5:0):1;
    }
    return {x,y:0,width:word.width,height:word.height,opacity};
  });
  return {words:targets,width,height,opacity:progress===0?1:0,presentWordCount:count,
    // Expanding lines retain the full line's anchor; centered lines re-anchor.
    anchorWidth:options.wordsReveal==='line-by-line'&&growth==='left-anchored'?group.width:width};
}
