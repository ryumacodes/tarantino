import {it,expect} from 'vitest';
import {windowShutter} from './windowShutter';
import {previewCameraPose} from './previewCamera';
const layout={width:1000,height:600,planeWidth:8,planeHeight:4,pixelsPerUnit:100};
const motion={continuous:true,samplesPerSecond:120,rate:1,pan:1,zoom:1};
it('maps scene coordinates to top-left output pixels with a fixed shutter duration',()=>{
 const before=previewCameraPose(1.5,.3,.5,true,8,4),now=previewCameraPose(1.5,.7,.5,true,8,4);
 const shutter=windowShutter(now,before,layout,motion);
 expect(shutter.now).toEqual([expect.closeTo(420),300,1200,600]);
 expect(shutter.before).toEqual([expect.closeTo(740),300,1200,600]);
 expect(shutter.active).toBe(true);
});
it('independently controls pan/zoom strength and playback speed',()=>{
 const before=previewCameraPose(1.4,.3,.5,true,8,4),now=previewCameraPose(1.5,.7,.5,true,8,4);
 const shutter=windowShutter(now,before,layout,{...motion,pan:0,zoom:.5,rate:2});
 expect(shutter.before.slice(0,2)).toEqual(shutter.now.slice(0,2));
 expect(shutter.before[2]).toBeCloseTo(1040);
});
it('does not blur across pauses, initial frames or clip boundaries',()=>{
 const before=previewCameraPose(1,.2,.3,true,8,4),now=previewCameraPose(3,.8,.7,true,8,4);
 const shutter=windowShutter(now,before,layout,{...motion,continuous:false});
 expect(shutter.before).toEqual(shutter.now);expect(shutter.active).toBe(false);
});
