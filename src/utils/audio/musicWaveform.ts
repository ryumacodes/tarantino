import { peakIndex } from './peakIndex';
/** Sample a looping soundtrack on edited time; source clip speed never enters this clock. */
export function musicPeak(peaks:readonly number[],durationMs:number,timeMs:number,rate=100){
  if(!peaks.length || durationMs<=0 || !Number.isFinite(durationMs))return 0;
  const phase=((timeMs%durationMs)+durationMs)%durationMs;
  return peaks[Math.min(peaks.length-1,Math.floor(phase*rate/1000))] ?? 0;
}

export function musicPeakRange(peaks:readonly number[],durationMs:number,startMs:number,endMs:number,maximum:number,rate=100){
  if(!peaks.length || durationMs<=0 || endMs<=startMs)return 0;
  if(endMs-startMs>=durationMs)return maximum;
  const start=((startMs%durationMs)+durationMs)%durationMs;
  const end=start+endMs-startMs;
  let level=0;
  const index=peakIndex(peaks);
  const scan=(a:number,b:number)=>{
    level=Math.max(level,index.range(a*rate/1000,b*rate/1000));
  };
  scan(start,Math.min(end,durationMs));
  if(end>durationMs)scan(0,end-durationMs);
  return level;
}
