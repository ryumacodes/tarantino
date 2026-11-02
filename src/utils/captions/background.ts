import {captionCorner,type CaptionCorner} from './corners';
/** Trace the same radial corner field used by native export; layout bounds stay unchanged. */
function path(ctx:CanvasRenderingContext2D,width:number,height:number,corner:CaptionCorner,outline:boolean) {
  const shift=outline?5:0,threshold=outline?5:0,r=corner.radius;
  ctx.beginPath();
  // Clockwise corners. Interpolation follows the compositor's 16-entry table.
  for(let quadrant=0;quadrant<4;quadrant++)for(let step=0;step<=96;step++) {
    const angle=step/96*Math.PI/2;
    const sample=Math.min(angle,Math.PI/2-angle)*60/Math.PI;
    const low=Math.min(15,Math.floor(sample)),high=Math.min(15,low+1);
    const ratio=corner.curvature[low]+(corner.curvature[high]-corner.curvature[low])*(sample-low);
    const distance=r*ratio+threshold;
    const x=distance*Math.sin(angle),y=-distance*Math.cos(angle);
    const centerX=quadrant===0||quadrant===1?width-r+shift:r-shift;
    const centerY=quadrant===0||quadrant===3?r-shift:height-r+shift;
    const rotation=quadrant*Math.PI/2;
    const px=centerX+x*Math.cos(rotation)-y*Math.sin(rotation);
    const py=centerY+x*Math.sin(rotation)+y*Math.cos(rotation);
    if(quadrant===0&&step===0)ctx.moveTo(px,py);else ctx.lineTo(px,py);
  }
  ctx.closePath();
}
export function createCaptionBackground(color:string) {
  const canvas=document.createElement('canvas'),ctx=canvas.getContext('2d');
  if(!ctx)throw new Error('Could not render caption background');
  let key='';
  return (target:CanvasRenderingContext2D,x:number,y:number,width:number,height:number)=>{
    if(width<=0||height<=0)return;
    const next=`${width}:${height}`;
    if(next!==key){
      const w=Math.ceil((width+24)*2),h=Math.ceil((height+24)*2);
      if(w>16384||h>16384||w*h*4>128*1024*1024)throw new Error('Caption background exceeds raster size limit');
      canvas.width=w;canvas.height=h;
      ctx.setTransform(2,0,0,2,24,24);
      const corner=captionCorner(width,height);
      const rgb=[1,3,5].map(index=>255-parseInt(color.slice(index,index+2),16));
      ctx.fillStyle=`rgba(${rgb.join(',')},0.1)`;
      path(ctx,width,height,corner,true);ctx.fill();
      ctx.fillStyle=color;path(ctx,width,height,corner,false);ctx.fill();
      key=next;
    }
    // Composite the group once so a fading opaque fill does not reveal its own outline.
    target.drawImage(canvas,x-12,y-12,canvas.width/2,canvas.height/2);
  };
}
