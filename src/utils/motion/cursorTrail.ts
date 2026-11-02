
/** Reconstruct fixed 60Hz history from the 120Hz simulation, independent of rendering. */
export function cursorTrailSamples(frames:readonly (readonly number[])[],timeMs:number,startMs:number,length:number,firstVisibleMs=startMs) {
  if(!frames.length || timeMs<startMs || !Number.isFinite(timeMs))return [];
  const count=Math.min(30,Math.max(0,Math.floor(length)));
  const last=Math.floor((timeMs-startMs)*60/1000+1e-7);
  const result:{time:number;frame:readonly number[]}[]=[];
  for(let tick=Math.max(0,last-count+1);tick<=last;tick++) {
    const time=startMs+tick*1000/60;
    const index=Math.max(0,Math.min(frames.length-1,Math.ceil(time*120/1000-1e-7)));
    const frame=frames[index];
    // No trail before the first visible cursor sample.
    if(frame && time+1e-7>=firstVisibleMs)result.push({time,frame});
  }
  return result;
}
