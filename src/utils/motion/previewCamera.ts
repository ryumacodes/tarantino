import { displayZoomView } from './displayZoomView';
import type { SourceCrop } from '../sourceCrop';
import type { CameraFrame } from './zoomTrajectory';

export interface PreviewCameraPose {
  scale: number;
  scaleY?: number;
  offsetX: number;
  offsetY: number;
  view: SourceCrop;
}

export function previewCameraFramePose(frame: CameraFrame, windowMode: boolean,
  planeWidth: number, planeHeight: number): PreviewCameraPose {
  const [scale,centerX,centerY,x,y,width,height] = frame;
  if (windowMode && x!==undefined && y!==undefined && width!==undefined && height!==undefined
    && width>0 && height>0) {
    return {scale:width,scaleY:height,offsetX:(x+width/2-.5)*planeWidth,
      offsetY:(.5-y-height/2)*planeHeight,view:{x:0,y:0,width:1,height:1}};
  }
  return previewCameraPose(scale,centerX,centerY,windowMode,planeWidth,planeHeight);
}

/** The same mapping drives drawing, cursor placement and motion measurement. */
export function previewCameraPose(scale: number, centerX: number, centerY: number,
  windowMode: boolean, planeWidth: number, planeHeight: number): PreviewCameraPose {
  const view = displayZoomView(scale, centerX, centerY);
  let offsetX = (0.5-view.x-view.width/2)/view.width*planeWidth;
  let offsetY = -(0.5-view.y-view.height/2)/view.height*planeHeight;
  if (windowMode && scale > 1) {
    const t = Math.max(0, Math.min(1, scale-1));
    const strength = t*t*(3-2*t);
    const anchor = (value: number) => value+(Math.max(.22,Math.min(.78,value))-value)*strength;
    offsetX = (anchor(centerX)-.5)*planeWidth-(centerX-.5)*planeWidth*scale;
    offsetY = (.5-anchor(centerY))*planeHeight-(.5-centerY)*planeHeight*scale;
  }
  return {scale:windowMode ? scale : 1/view.width,offsetX,offsetY,
    view:windowMode ? {x:0,y:0,width:1,height:1} : view};
}

/** Derivative of the actual projected camera, in source seconds, scaled by clip speed. */
export function previewCameraVelocity(now: PreviewCameraPose, before: PreviewCameraPose,
  planeWidth: number, planeHeight: number, samplesPerSecond: number, rate: number,
  continuous: boolean) {
  if (!continuous || planeWidth<=0 || planeHeight<=0 || samplesPerSecond<=0) {
    return {scale:0,x:0,y:0};
  }
  const frequency=samplesPerSecond*rate;
  const x=-(now.offsetX-before.offsetX)/planeWidth*frequency;
  const y=(now.offsetY-before.offsetY)/planeHeight*frequency;
  // Relative scale velocity keeps equal percentage zooms consistent at every zoom level.
  const scale=(now.scale-before.scale)/Math.max(now.scale,.1)*frequency;
  const moving=Math.hypot(x,y)>.15;
  return {scale,x:moving?x*10:0,y:moving?y*10:0};
}
