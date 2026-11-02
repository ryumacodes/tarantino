import type {PreviewCameraPose} from './previewCamera';
import {windowShutter,type WindowRect} from './windowShutter';

export interface PreviewCameraMotion {
  frames?:readonly import('./zoomTrajectory').CameraFrame[];
  now:PreviewCameraPose;before:PreviewCameraPose;
  beforeSourceTime:number|null;beforeCursorHidden:boolean;
  fps:number;continuous:boolean;timelineTime:number;revision:number;
}
export interface CursorProjection {
  width:number;height:number;viewportWidth:number;viewportHeight:number;
  planeWidth:number;planeHeight:number;size:number;windowMode:boolean;
}

/** Cursor coordinates are top-left stage pixels, matching native export. */
export function projectCursorPose(frame:readonly number[],camera:PreviewCameraPose,layout:CursorProjection):WindowRect {
  return [
    layout.width/2+((frame[0]-.5)*layout.planeWidth*camera.scale+camera.offsetX)*layout.width/layout.viewportWidth,
    layout.height/2-((.5-frame[1])*layout.planeHeight*camera.scale+camera.offsetY)*layout.height/layout.viewportHeight,
    layout.size*(1-(frame[9]??0))*(layout.windowMode?camera.scale:1),frame[7]??0,
  ];
}

export function cursorShutter(frame:readonly number[],previous:readonly number[],camera:PreviewCameraMotion,
  layout:CursorProjection,pan:number,zoom:number,enabled:boolean) {
  const now=projectCursorPose(frame,camera.now,layout),old=projectCursorPose(previous,camera.before,layout);
  const before:WindowRect=[...now];
  const active=enabled&&camera.continuous&&!camera.beforeCursorHidden&&frame[2]>.01&&previous[2]>.01;
  if(active) {
    const translation=pan*camera.fps/60,scale=zoom*camera.fps/60;
    before[0]+=(old[0]-now[0])*translation;before[1]+=(old[1]-now[1])*translation;
    before[2]=Math.max(.001,now[2]+(old[2]-now[2])*scale);
    const angle=((old[3]-now[3]+180)%360+360)%360-180;
    before[3]+=angle*translation;
  }
  const fixed:WindowRect=[layout.width/2,layout.height/2,
    layout.planeWidth*layout.width/layout.viewportWidth,layout.planeHeight*layout.height/layout.viewportHeight];
  const clips=windowShutter(camera.now,camera.before,{width:layout.width,height:layout.height,
    planeWidth:layout.planeWidth,planeHeight:layout.planeHeight,pixelsPerUnit:layout.width/layout.viewportWidth},
    {continuous:active,samplesPerSecond:camera.fps,rate:1,pan,zoom});
  return {before,now,active,clipBefore:layout.windowMode?clips.before:fixed,clipNow:layout.windowMode?clips.now:fixed};
}

/** Native cursor size is expressed in output pixels, independent of source padding/crop. */
export function cursorOutputScale(width:number,height:number,outputWidth:number,outputHeight:number) {
  if(width<=0||height<=0||outputWidth<=0||outputHeight<=0)return 0;
  return Math.min(width/outputWidth,height/outputHeight);
}
