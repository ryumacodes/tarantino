import {expect,it} from 'vitest';
import {previewCameraFramePose,previewCameraPose} from './previewCamera';
import {windowShutter} from './windowShutter';
import {projectCursorPose} from './cursorShutter';
import {projectLoupeToPreview} from './loupe';

it('uses the same rectangle for window, cursor, lens and shutter geometry',()=>{
  const pose=previewCameraFramePose([3,.9,.9,.125,.25,.5,.25],true,128,128);
  expect(pose).toEqual({scale:.5,scaleY:.25,offsetX:-16,offsetY:16,
    view:{x:0,y:0,width:1,height:1}});
  const shutter=windowShutter(pose,pose,
    {width:128,height:128,planeWidth:128,planeHeight:128,pixelsPerUnit:1},
    {continuous:false,samplesPerSecond:60,rate:1,pan:1,zoom:1});
  expect(shutter.now).toEqual([48,48,64,32]);
  const cursor=projectCursorPose([.75,.5,1,0,0,0,0,0,0,0],pose,
    {width:128,height:128,viewportWidth:128,viewportHeight:128,
      planeWidth:128,planeHeight:128,size:20,windowMode:true});
  expect(cursor).toEqual([64,48,10,0]);
  const lens=projectLoupeToPreview({center_x:.75,center_y:.5,radius:.125,zoom:2,
    presence:1,bevel:0,chromatic_aberration:0,shadow_alpha:0},
    {width:128,height:128},undefined,
    {...pose,viewportWidth:128,viewportHeight:128,planeWidth:128,planeHeight:128});
  expect([lens.x,lens.y,lens.radius]).toEqual([.5,.625,.0625]);
});

it('preserves legacy poses when rectangle data is absent',()=>{
  for(const window of [false,true]) {
    const expected=previewCameraPose(2,.7,.3,window,128,64);
    expect(previewCameraFramePose([2,.7,.3],window,128,64)).toEqual(expected);
    expect(previewCameraFramePose([2,.7,.3,0,0,0,0],window,128,64)).toEqual(expected);
  }
});
