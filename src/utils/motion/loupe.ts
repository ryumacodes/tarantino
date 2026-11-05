import { cropSourcePoint, resolveSourceCrop, type SourceCrop } from '../sourceCrop';

export interface LoupeFrame {
  center_x: number;
  center_y: number;
  radius: number;
  zoom: number;
  presence: number;
  bevel: number;
  chromatic_aberration: number;
  shadow_alpha: number;
}

export interface VideoTransform {
  scale: number;
  scaleY?: number;
  offsetX: number;
  offsetY: number;
  viewportWidth: number;
  viewportHeight: number;
  planeWidth: number;
  planeHeight: number;
}

/** WebGL UV has a bottom-left origin; native targets use original source units. */
export function projectLoupeToPreview(frame: LoupeFrame, source: {width:number;height:number},
  sourceCrop: SourceCrop | undefined, transform: VideoTransform) {
  const crop = resolveSourceCrop(sourceCrop ?? {x:0,y:0,width:1,height:1},source.width,source.height);
  const point = cropSourcePoint(crop,frame.center_x,frame.center_y);
  const lightX = (0.5-frame.center_x)*source.width;
  const lightY = (-0.5-frame.center_y)*source.height;
  const distance = Math.hypot(lightX,lightY);
  return {
    x: 0.5+((point.x-0.5)*transform.planeWidth*transform.scale+transform.offsetX)/transform.viewportWidth,
    y: 0.5+((0.5-point.y)*transform.planeHeight*(transform.scaleY??transform.scale)+transform.offsetY)/transform.viewportHeight,
    lightDirection: distance < 0.001 ? [0,1] : [lightX/distance,-lightY/distance],
    shineExponent: Math.max(1.5,Math.min(24,distance/(0.1*source.width))),
    lightTilt: Math.min(2,distance/(0.8*source.width)),
    sourcePixelScale: transform.planeWidth*transform.scale/crop.width/transform.viewportWidth,
    shadowReach: 0.9*source.width/crop.width*transform.planeWidth*transform.scale/transform.viewportWidth,
    radius: frame.radius*source.width/crop.width*transform.planeWidth*transform.scale/transform.viewportWidth,
  };
}
