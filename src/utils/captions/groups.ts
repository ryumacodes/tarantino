import type {TimedTranscriptText} from './timing';
export interface TextMeasurement {width:number;height:number}
export type MeasuredWord<T> = T&TextMeasurement;
export interface CaptionGroup<T> {
  words:MeasuredWord<T>[];
  offsets:number[];
  heights:number[];
  startMs:number;endMs:number;width:number;height:number;
}
/** Group in source time before projecting through timeline edits. Measurement
 * comes from the renderer's font, shared by preview and export preparation. */
export function captionGroups<T extends TimedTranscriptText>(
  words:readonly T[],measure:(text:string)=>TextMeasurement,maxWidth:number,
):CaptionGroup<T>[] {
  if(!Number.isFinite(maxWidth)||maxWidth<=0)throw new RangeError('Invalid caption width');
  const measured=(word:T):MeasuredWord<T>=>{
    const size=measure(word.text);
    if(!Number.isFinite(size.width)||!Number.isFinite(size.height)||size.width<0||size.height<0)
      throw new RangeError('Invalid caption measurement');
    return {...word,...size};
  };
  const space=measured({text:' ',startMs:0,endMs:0} as T).width;
  const result:CaptionGroup<T>[]=[];
  let pending:MeasuredWord<T>[]=[],characters=0,width=0;
  function flush() {
    if(!pending.length)return;
    const offsets=[0],heights=[0];
    const line=pending.map((word,index)=>{
      let text=word.text.trimStart();
      if(index>0&&index===pending.length-1)text=text.trimEnd();
      const cap=measured({...word,text});
      if(index>0){cap.text=' '+cap.text;cap.width+=space;}
      offsets.push(offsets[index]+cap.width);heights.push(Math.max(heights[index],cap.height));
      return cap;
    });
    result.push({words:line,offsets,heights,startMs:line[0].startMs,endMs:line[line.length-1].endMs,
      width:offsets[offsets.length-1],height:heights[heights.length-1]});
  }
  for(const original of words) {
    const word=measured(original),first=pending[0];
    const exceeds=characters+word.text.length>42||width+(first?space:0)+word.width>maxWidth
      ||(first&&(word.endMs-first.startMs>7000||word.startMs-first.endMs>1000));
    if(exceeds) {
      if(!first)continue;
      flush();pending=[word];characters=word.text.length;width=word.width;
    }else{pending.push(word);characters+=word.text.length;width+=word.width;}
  }
  flush();
  result.forEach((group,index)=>{
    group.endMs=Math.min(group.endMs+300,result[index+1]?.startMs??Infinity);
    group.words[group.words.length-1].endMs=group.endMs;
  });
  return result;
}
