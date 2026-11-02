import type { SourceCrop } from '../sourceCrop';

/** Source interval sampled inside the fixed display viewport by the compositor. */
export function displayZoomView(scale: number, centerX: number, centerY: number): SourceCrop {
  if (scale <= 1.001) return {x:0,y:0,width:1,height:1};
  const visible = 1/scale;
  return {x:Math.max(0,Math.min(1-visible,centerX-visible/2)),
    y:Math.max(0,Math.min(1-visible,centerY-visible/2)),width:visible,height:visible};
}
