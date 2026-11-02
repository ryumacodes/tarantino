import {it,expect} from 'vitest';
import {cursorShutter,projectCursorPose,cursorOutputScale,type PreviewCameraMotion} from './cursorShutter';
import {previewCameraPose} from './previewCamera';
const layout={width:128,height:128,viewportWidth:128,viewportHeight:128,planeWidth:64,planeHeight:64,size:1,windowMode:true};
const camera:PreviewCameraMotion={now:previewCameraPose(1.5,.7,.5,true,64,64),before:previewCameraPose(1.2,.3,.5,true,64,64),beforeSourceTime:0,beforeCursorHidden:false,fps:60,continuous:true,timelineTime:16,revision:1};
const old=[.1,.6,.6,0,0,0,0,-30],now=[.9,.4,.6,0,0,0,0,45];
it('projects cursor position and size through the window camera before integrating motion',()=>{
 const value=cursorShutter(now,old,camera,layout,1,1,true);
 expect(value.before).toEqual([expect.closeTo(35.84),expect.closeTo(71.68),1.2,-30]);
 expect(value.now).toEqual([expect.closeTo(96),expect.closeTo(54.4),1.5,45]);
 expect(value.clipBefore).toEqual([expect.closeTo(66.56),64,expect.closeTo(76.8),expect.closeTo(76.8)]);
});
it('keeps display cursor size and crop rectangle fixed while projecting camera position',()=>{
 const display={...layout,windowMode:false};
 const value=cursorShutter(now,old,camera,display,1,1,true);
 expect(value.now[2]).toBe(1);expect(value.before[2]).toBe(1);
 expect(value.clipBefore).toEqual([64,64,64,64]);expect(value.clipNow).toEqual(value.clipBefore);
});
it('uses the shortest wrapped angle and separates pan from zoom strength',()=>{
 const a=[...old],b=[...now];a[7]=179;b[7]=-179;
 const value=cursorShutter(b,a,camera,layout,1,0,true);
 expect(value.before[3]-value.now[3]).toBe(-2);
 expect(value.before[2]).toBe(value.now[2]);
 const zoom=cursorShutter(now,old,camera,layout,0,1,true);
 expect(zoom.before.slice(0,2)).toEqual(zoom.now.slice(0,2));expect(zoom.before[3]).toBe(45);
});
it('resets hidden, discontinuous and disabled motion',()=>{
 for(const sample of [{...camera,continuous:false},{...camera,beforeCursorHidden:true}]){
  const value=cursorShutter(now,old,sample,layout,1,1,true);
  expect(value.active).toBe(false);expect(value.before).toEqual(value.now);
 }
 for(const [current,previous,enabled] of [[now,[.1,.6,0],true],[[.9,.4,0],old,true],[now,old,false]] as const){
  const value=cursorShutter(current,previous,camera,layout,1,1,enabled);
  expect(value.active).toBe(false);expect(value.before).toEqual(projectCursorPose(current,camera.now,layout));
 }
});

it('scales output-pixel cursor size to the fitted framebuffer without source-size dependence',()=>{
 expect(cursorOutputScale(960,540,1920,1080)).toBe(.5);
 expect(cursorOutputScale(1920,1080,1920,1080)).toBe(1);
 expect(cursorOutputScale(1000,500,1000,1000)).toBe(.5);
 expect(cursorOutputScale(0,0,1920,1080)).toBe(0);
});

it('includes press scale in both shutter endpoints without moving the hotspot',()=>{
 const pressed=[...now,0,.1],released=[...old,0,0];
 for(const windowMode of [true,false]) {
  const projection={...layout,windowMode};
  const value=cursorShutter(pressed,released,camera,projection,1,1,true);
  const baseline=cursorShutter(now,old,camera,projection,1,1,true);
  expect(value.now[2]).toBeCloseTo(baseline.now[2]*.9);
  expect(value.before[2]).toBeCloseTo(baseline.before[2]);
  expect(value.now.slice(0,2)).toEqual(baseline.now.slice(0,2));
  expect(value.before.slice(0,2)).toEqual(baseline.before.slice(0,2));
 }
});
