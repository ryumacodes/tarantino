import type {RecordingMask} from './document';
import type {PixelCrop} from '../sourceCrop';
/** Map creation-crop pixels into the current decoded crop, preserving offscreen bounds. */
export function maskPixelBounds(mask:RecordingMask,crop:PixelCrop){
 const rect=mask.bounds[0],basis=mask.coordinateSpace;
 return {x:rect.x+(basis?basis.x-crop.x:0),y:rect.y+(basis?basis.y-crop.y:0),width:rect.width,height:rect.height};
}
