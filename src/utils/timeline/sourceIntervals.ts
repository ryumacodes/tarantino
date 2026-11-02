import {timelineDuration,visibleTimelineSegments,type TimelineState} from './previewTimeline';
export interface SourceInterval {startMs:number;endMs:number}

/** Build once per edit; queries retain overlapping source intervals. */
export class SourceIntervalIndex<T extends SourceInterval> {
  private entries:{word:T;index:number;maxEnd:number}[];
  constructor(words:readonly T[]) {
    let maxEnd=0;
    this.entries=words.map((word,index)=>{
      if(!Number.isFinite(word.startMs)||!Number.isFinite(word.endMs)||word.startMs<0||word.endMs<word.startMs)
        throw new RangeError('Invalid source interval');
      return {word,index,maxEnd:0};
    }).sort((a,b)=>a.word.startMs-b.word.startMs||a.index-b.index);
    for(const entry of this.entries)entry.maxEnd=maxEnd=Math.max(maxEnd,entry.word.endMs);
  }
  intersect(start:number,end:number):{word:T;index:number}[] {
    if(!Number.isFinite(start)||!Number.isFinite(end)||end<=start)return [];
    let low=0,high=this.entries.length;
    while(low<high){const mid=(low+high)>>>1;if(this.entries[mid].maxEnd<=start)low=mid+1;else high=mid;}
    const result:{word:T;index:number}[]=[];
    for(let i=low;i<this.entries.length&&this.entries[i].word.startMs<end;i++) {
      const entry=this.entries[i];
      if(entry.word.endMs>start&&entry.word.endMs>entry.word.startMs)result.push(entry);
    }
    return result;
  }
}
export interface SourceOccurrence<T extends SourceInterval> {
  word:T;wordIndex:number;clipId:string|null;sourcePath:string|undefined;
  startMs:number;endMs:number;sourceStartMs:number;sourceEndMs:number;
}

/** Project source intervals through the same visible clip selection as playback.
 * Result times are relative to export trim; intervals are half-open. Resolve each
 * recording separately so repeated edits work without leaking another source's intervals.
 */
export function projectSourceIntervals<T extends SourceInterval>(
  state:TimelineState&{trimStart:number;trimEnd:number},
  resolve:(sourcePath:string|undefined)=>SourceIntervalIndex<T>|undefined,
):SourceOccurrence<T>[] {
  const start=Math.max(0,state.trimStart),end=Math.min(state.trimEnd,timelineDuration(state));
  if(!Number.isFinite(start)||!Number.isFinite(end)||end<=start)return [];
  const segments=state.tracks.some(track=>track.type==='video'||track.type==='screen')
    ?visibleTimelineSegments(state).map(({start,end,entry:{clip}})=>({start,end,clipId:clip.id,
      sourcePath:clip.sourceFilePath,sourceIn:clip.sourceIn,clipStart:clip.startTime,rate:clip.playbackRate}))
    :[{start:0,end:state.duration,clipId:null,sourcePath:undefined,sourceIn:0,clipStart:0,rate:1}];
  const result:SourceOccurrence<T>[]=[];
  for(const segment of segments) {
    const from=Math.max(start,segment.start),to=Math.min(end,segment.end);
    if(to<=from||!Number.isFinite(segment.rate)||segment.rate<=0)continue;
    const sourceFrom=segment.sourceIn+(from-segment.clipStart)*segment.rate;
    const sourceTo=segment.sourceIn+(to-segment.clipStart)*segment.rate;
    const transcript=resolve(segment.sourcePath);if(!transcript)continue;
    for(const {word,index} of transcript.intersect(sourceFrom,sourceTo)) {
      const sourceStartMs=Math.max(sourceFrom,word.startMs),sourceEndMs=Math.min(sourceTo,word.endMs);
      result.push({word,wordIndex:index,clipId:segment.clipId,sourcePath:segment.sourcePath,
        startMs:from-start+(sourceStartMs-sourceFrom)/segment.rate,
        endMs:from-start+(sourceEndMs-sourceFrom)/segment.rate,sourceStartMs,sourceEndMs});
    }
  }
  return result;
}
