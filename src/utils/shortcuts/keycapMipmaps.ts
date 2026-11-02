export interface KeycapMip {width:number;height:number;pixels:Uint8Array}
/** Premultiplied RGBA8 pyramid: box first, then separable binomial filtering. */
export function keycapMipmaps(width:number,height:number,rgba:Uint8ClampedArray|Uint8Array):KeycapMip[] {
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||rgba.length!==width*height*4)
    throw new RangeError('Invalid keycap raster');
  const pixels=new Uint8Array(rgba.length);
  for(let i=0;i<rgba.length;i+=4) {
    for(let c=0;c<3;c++)pixels[i+c]=Math.round(rgba[i+c]*rgba[i+3]/255);
    pixels[i+3]=rgba[i+3];
  }
  const levels:KeycapMip[]=[{width,height,pixels}];
  while(width>1||height>1) {
    const source=levels[levels.length-1],first=levels.length===1;
    width=Math.max(1,Math.floor(width/2));height=Math.max(1,Math.floor(height/2));
    const pixels=new Uint8Array(width*height*4),weights=first?[.5,.5]:[.125,.375,.375,.125];
    for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
      const color=[0,0,0,0],originX=x*2-(first?0:1),originY=y*2-(first?0:1);
      for(let dy=0;dy<weights.length;dy++)for(let dx=0;dx<weights.length;dx++) {
        const sx=Math.max(0,Math.min(source.width-1,originX+dx));
        const sy=Math.max(0,Math.min(source.height-1,originY+dy));
        const index=(sy*source.width+sx)*4,weight=weights[dx]*weights[dy];
        for(let c=0;c<4;c++)color[c]+=source.pixels[index+c]*weight;
      }
      for(let c=0;c<4;c++)pixels[(y*width+x)*4+c]=Math.round(color[c]);
    }
    levels.push({width,height,pixels});
  }
  return levels;
}
/** WebGL typed-array uploads do not apply UNPACK_FLIP_Y_WEBGL. */
export function bottomUpPixels(level:KeycapMip) {
  const output=new Uint8Array(level.pixels.length),stride=level.width*4;
  for(let y=0;y<level.height;y++)output.set(level.pixels.subarray(y*stride,(y+1)*stride),(level.height-y-1)*stride);
  return output;
}
