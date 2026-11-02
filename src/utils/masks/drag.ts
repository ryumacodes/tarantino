import type {MaskBounds} from './document';
export interface MaskPoint{x:number;y:number}
export type MaskDragMode='draw'|'move'|'nw'|'ne'|'sw'|'se';
const clamp=(value:number,min:number,max:number)=>Math.max(min,Math.min(max,value));
/** Coordinates are cropped source pixels, independent of canvas zoom and display scale. */
export function draggedMaskBounds(original:MaskBounds,anchor:MaskPoint,point:MaskPoint,mode:MaskDragMode,width:number,height:number):MaskBounds{
 if(![...Object.values(original),anchor.x,anchor.y,point.x,point.y,width,height].every(Number.isFinite)
  ||width<1||height<1||original.width<=0||original.height<=0)throw new RangeError('Invalid mask drag geometry');
 const current={x:clamp(point.x,0,width),y:clamp(point.y,0,height)};
 if(mode==='move')return {...original,x:clamp(original.x+point.x-anchor.x,0,Math.max(0,width-original.width)),
  y:clamp(original.y+point.y-anchor.y,0,Math.max(0,height-original.height))};
 const fixed=mode==='draw'?{x:clamp(anchor.x,0,width),y:clamp(anchor.y,0,height)}:
  {x:clamp(mode.endsWith('w')?original.x+original.width:original.x,0,width),
   y:clamp(mode.startsWith('n')?original.y+original.height:original.y,0,height)};
 const left=Math.min(fixed.x,current.x),top=Math.min(fixed.y,current.y);
 return {x:Math.min(left,width-1),y:Math.min(top,height-1),
  width:Math.max(1,Math.abs(current.x-fixed.x)),height:Math.max(1,Math.abs(current.y-fixed.y))};
}
