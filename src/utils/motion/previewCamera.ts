import { displayZoomView } from './displayZoomView';
import type { SourceCrop } from '../sourceCrop';

export interface PreviewCameraPose {
  scale: number;
  offsetX: number;
  offsetY: number;
  view: SourceCrop;
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
