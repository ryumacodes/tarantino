import {it,expect} from 'vitest';
import {continuousPreviewMotion,type PreviewMotionSample} from './previewContinuity';
const base:PreviewMotionSample={playing:true,revision:1,clipId:'clip',sourceTime:1000,sourcePath:'recording.mp4',rate:1,trajectory:[],clips:[],tracks:[]};
it('preserves motion across ordinary ticks and repeated render frames',()=>{
 expect(continuousPreviewMotion(base,{...base,sourceTime:1016})).toBe(true);
 expect(continuousPreviewMotion(base,{...base})).toBe(true);
});
it('resets forward seeks inside the same clip even if the jump is only one millisecond',()=>{
 expect(continuousPreviewMotion(base,{...base,sourceTime:1001,revision:2})).toBe(false);
 expect(continuousPreviewMotion(base,{...base,sourceTime:5000,revision:2})).toBe(false);
});
it('resets pause/resume, backward jumps, source changes, cuts, gaps and edited motion',()=>{
 const changes:Partial<PreviewMotionSample>[]=[{playing:false},{sourceTime:999},{sourceTime:null},{clipId:'next'},
  {sourcePath:'other.mp4'},{rate:2},{trajectory:[]},{clips:[]},{tracks:[]},{revision:2}];
 for(const change of changes)expect(continuousPreviewMotion(base,{...base,...change})).toBe(false);
 expect(continuousPreviewMotion(null,base)).toBe(false);
 expect(continuousPreviewMotion({...base,playing:false},base)).toBe(false);
 expect(continuousPreviewMotion({...base,sourceTime:null},base)).toBe(false);
});
