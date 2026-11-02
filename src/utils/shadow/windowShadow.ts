export interface ShadowField {
  width: number; height: number;
  rect: { x: number; y: number; width: number; height: number };
  radius: number; blur: number; intensity: number;
}

/** Separable clamped box filter. Its cost does not grow with blur radius. */
export function blurShadow(mask: Float32Array, width: number, height: number, radius: number) {
  radius=Math.max(0,Math.trunc(radius));
  if(!radius || !width || !height) return;
  const temporary=new Float32Array(mask.length);
  const lines=(input:Float32Array,output:Float32Array,length:number,count:number,step:number,lineStep:number)=>{
    const divisor=radius*2+1;
    for(let line=0;line<count;line++) {
      const base=line*lineStep;
      let sum=input[base]*(radius+1);
      for(let i=1;i<=Math.min(radius,length-1);i++) sum+=input[base+i*step];
      sum+=input[base+(length-1)*step]*Math.max(0,radius-length+1);
      for(let i=0;i<length;i++) {
        output[base+i*step]=sum/divisor;
        sum+=input[base+Math.min(i+radius+1,length-1)*step]-input[base+Math.max(0,i-radius)*step];
      }
    }
  };
  lines(mask,temporary,width,height,1,width);
  lines(temporary,mask,height,width,width,1);
}

/** Black RGBA shadow in top-left output pixels, matching export's field. */
export function windowShadowPixels({width,height,rect,radius,blur,intensity}:ShadowField) {
  const mask=new Float32Array(width*height),pixels=new Uint8Array(width*height*4);
  const corners=[[radius,radius],[rect.width-radius,radius],
    [radius,rect.height-radius],[rect.width-radius,rect.height-radius]];
  for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
    const lx=x-rect.x,ly=y-rect.y;
    if(lx<0||ly<0||lx>=rect.width||ly>=rect.height)continue;
    let alpha=1;
    for(const [cx,cy] of corners) {
      if((cx<rect.width/2?lx<cx:lx>cx)&&(cy<rect.height/2?ly<cy:ly>cy)) {
        alpha=1-Math.max(0,Math.min(1,Math.hypot(lx-cx,ly-cy)-radius+.5));break;
      }
    }
    mask[y*width+x]=alpha*intensity;
  }
  blurShadow(mask,width,height,blur);
  for(let i=0;i<mask.length;i++)pixels[i*4+3]=Math.max(0,Math.min(255,mask[i]*255));
  return pixels;
}
