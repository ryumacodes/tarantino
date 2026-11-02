import type {PreviewCameraPose} from './previewCamera';
export type WindowRect = [number,number,number,number];

/** Convert scene poses to top-left output pixels and a 1/60-second shutter. */
export function windowShutter(now:PreviewCameraPose,before:PreviewCameraPose,
  layout:{width:number;height:number;planeWidth:number;planeHeight:number;pixelsPerUnit:number},
  motion:{continuous:boolean;samplesPerSecond:number;rate:number;pan:number;zoom:number}) {
  const rect=(pose:PreviewCameraPose):WindowRect=>[
    layout.width/2+pose.offsetX*layout.pixelsPerUnit,
    layout.height/2-pose.offsetY*layout.pixelsPerUnit,
    layout.planeWidth*layout.pixelsPerUnit*pose.scale,
    layout.planeHeight*layout.pixelsPerUnit*pose.scale,
  ];
  const current=rect(now),old=rect(before),start:WindowRect=[...current];
  if(motion.continuous)for(let i=0;i<4;i++) {
    const strength=(i<2?motion.pan:motion.zoom)*motion.samplesPerSecond*motion.rate/60;
    start[i]+=(old[i]-current[i])*strength;
  }
  start[2]=Math.max(.001,start[2]);start[3]=Math.max(.001,start[3]);
  return {before:start,now:current,active:start.some((v,i)=>Math.abs(v-current[i])>.01)};
}
