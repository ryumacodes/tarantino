import type {TranscriptSession,TranscriptWord} from './document';
import {normalizeTranscriptTiming} from './timing';
import {visibleTimelineSegments,type TimelineState} from '../timeline/previewTimeline';
import {captionGroups,type TextMeasurement} from './groups';

export interface CaptionSourceWord extends TranscriptWord {sessionId:string}
/** Filter before line grouping, so removed text cannot change visible line layout.
 * Global export trims crop playback later; grouping remains stable across trims. */
export function prepareCaptionWords(state:TimelineState,primaryPath:string,sourcePath:string,
  sessions:readonly TranscriptSession[],showSoundLabels:boolean):CaptionSourceWord[] {
  const words=sessions.filter(session=>session.sourceFilePath===sourcePath).flatMap(session=>
    normalizeTranscriptTiming(session.words,session.offsetMs).map(word=>({...word,sessionId:session.id})));
  const legacy=!state.tracks.some(track=>track.type==='video'||track.type==='screen');
  const ranges=legacy?(sourcePath===primaryPath?[{start:0,end:state.duration}]:[]):
    visibleTimelineSegments(state).filter(({entry:{clip}})=>(clip.sourceFilePath??primaryPath)===sourcePath)
      .map(({start,end,entry:{clip}})=>({start:clip.sourceIn+(start-clip.startTime)*clip.playbackRate,
        end:clip.sourceIn+(end-clip.startTime)*clip.playbackRate}));
  ranges.sort((a,b)=>a.start-b.start);
  // Merge repetitions/overlaps once rather than checking every clip for each word.
  const merged:{start:number;end:number}[]=[];
  for(const range of ranges){
    if(!Number.isFinite(range.start)||!Number.isFinite(range.end)||range.end<=range.start)continue;
    const last=merged[merged.length-1];
    if(last&&range.start<=last.end)last.end=Math.max(last.end,range.end);else merged.push({...range});
  }
  return words.filter(word=>{
    if(word.isHidden||(!showSoundLabels&&word.type==='sound-label')||word.endMs<=word.startMs)return false;
    let low=0,high=merged.length;
    while(low<high){const mid=(low+high)>>>1;if(merged[mid].end<=word.startMs)low=mid+1;else high=mid;}
    return low<merged.length&&merged[low].start<word.endMs;
  });
}
export function prepareCaptionGroups(state:TimelineState,primaryPath:string,sourcePath:string,
  sessions:readonly TranscriptSession[],showSoundLabels:boolean,measure:(text:string)=>TextMeasurement,stageWidth:number) {
  return captionGroups(prepareCaptionWords(state,primaryPath,sourcePath,sessions,showSoundLabels),measure,stageWidth*.8);
}
