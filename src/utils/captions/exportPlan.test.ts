import {expect,it,vi} from 'vitest';
import {captionExportPlan} from './exportPlan';
import type {CaptionScene} from './renderer';
const part={x:100,y:80,width:120,height:30,opacity:.5,blurRadius:5};
const scene:CaptionScene={background:part,words:[{text:'hello',target:part}]};
it('preserves blur and analytic background while placing padded text tiles',async()=>{
  const raster=vi.fn(()=>({tile:3,width:42,height:24,margin:2}));
  const result=await captionExportPlan(2,30,()=>scene,raster);
  expect(result).toEqual({frames:2,spans:[{start_frame:0,end_frame:2,background:part,
    words:[{...part,tile:3,x:98,y:78,width:42,height:24}]}]});
});
it('samples the output clock, preserves blank gaps and does not join separated spans',async()=>{
  const times:number[]=[];
  const result=await captionExportPlan(4,40,time=>{times.push(time);return time===25?null:scene;},
    ()=>({tile:0,width:120,height:30,margin:0}));
  expect(times).toEqual([0,25,50,75]);
  expect(result.spans.map(s=>[s.start_frame,s.end_frame])).toEqual([[0,1],[2,4]]);
});
it('avoids rasterizing invisible words and yields on long plans',async()=>{
  const raster=vi.fn(),yieldControl=vi.fn(async()=>{});
  const result=await captionExportPlan(481,60,()=>({background:{...part,opacity:0},words:[{text:'hidden',target:{...part,opacity:0}}]}),raster,yieldControl);
  expect(result.spans).toEqual([]);expect(raster).not.toHaveBeenCalled();expect(yieldControl).toHaveBeenCalledTimes(2);
});
it('rejects invalid export clocks',async()=>{
  for(const [frames,fps] of [[-1,30],[1,0],[1,Infinity],[1.5,30]])
    await expect(captionExportPlan(frames,fps,()=>null,vi.fn())).rejects.toThrow('clock');
});
