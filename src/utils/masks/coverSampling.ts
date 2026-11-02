import type {MaskBounds} from './document';
interface Size {width:number;height:number}
export interface MaskCoverSampling {
  lattice:MaskBounds;
  tapsPerQuadrant:{x:number;y:number};
  level:number;
  levelSpan:Size;
}
/** Plan a bounded 2×2 color summary in source-texture pixels. Each output texel
 * averages one quadrant; this is an opaque cover, not a blurred source image.
 */
export function maskCoverSampling(bounds:MaskBounds,uvRect:MaskBounds,size:Size,mipLevels:number):MaskCoverSampling|null {
  if(![...Object.values(bounds),...Object.values(uvRect),size.width,size.height,mipLevels].every(Number.isFinite)
    ||bounds.width<=0||bounds.height<=0||uvRect.width<=0||uvRect.height<=0
    ||size.width<1||size.height<1||!Number.isInteger(mipLevels)||mipLevels<1||mipLevels>32)
    throw new RangeError('Invalid mask sampling geometry');
  const left=Math.max(0,(bounds.x-uvRect.x)/uvRect.width*size.width);
  const top=Math.max(0,(bounds.y-uvRect.y)/uvRect.height*size.height);
  const right=Math.min(size.width,(bounds.x+bounds.width-uvRect.x)/uvRect.width*size.width);
  const bottom=Math.min(size.height,(bounds.y+bounds.height-uvRect.y)/uvRect.height*size.height);
  if(right<=left||bottom<=top)return null;
  const width=right-left,height=bottom-top;
  const maxLevel=Math.min(mipLevels-1,Math.max(0,Math.floor(Math.log2(Math.min(width,height)/8))));
  for(let level=0;level<=maxLevel;level++){
    const scale=2**level;
    // Box reduction at level one, then four-tap binomial reduction.
    const footprint=level===0?.5:1.5*scale-2;
    const insetX=Math.min(scale+footprint,width/2),insetY=Math.min(scale+footprint,height/2);
    const lattice={x:left+insetX,y:top+insetY,width:width-2*insetX,height:height-2*insetY};
    let x=Math.max(1,Math.ceil(lattice.width/(2*scale)));
    let y=Math.max(1,Math.ceil(lattice.height/(2*scale)));
    if(x*y>256&&level<maxLevel)continue;
    if(x*y>256){
      if(x>=y){x=Math.max(1,Math.floor(256/y));y=Math.min(y,256);}
      else {y=Math.max(1,Math.floor(256/x));x=Math.min(x,256);}
    }
    return {lattice,tapsPerQuadrant:{x,y},level,levelSpan:{
      width:Math.max(1,Math.floor(size.width/scale))*scale,
      height:Math.max(1,Math.floor(size.height/scale))*scale,
    }};
  }
  return null;
}
