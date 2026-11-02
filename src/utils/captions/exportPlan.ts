import type {CaptionScene} from './renderer';
import type {CaptionFramePart} from './motion';
export interface CaptionExportTile {tile:number;width:number;height:number;margin:number}
export interface CaptionExportWord extends CaptionFramePart {tile:number}
export interface CaptionExportSpan {
  start_frame:number;end_frame:number;background:CaptionFramePart|null;words:CaptionExportWord[];
}
/** Preserve analytic background geometry and per-part blur for native rendering.
 * Text tiles are reused; invisible words never need a raster. */
export async function captionExportPlan(frames:number,fps:number,sample:(time:number)=>CaptionScene|null,
  raster:(text:string)=>CaptionExportTile,
  yieldControl:()=>Promise<void>=()=>new Promise(resolve=>setTimeout(resolve,0))) {
  if(!Number.isSafeInteger(frames)||frames<0||!Number.isFinite(fps)||fps<=0||fps>240)
    throw new RangeError('Invalid caption export clock');
  const spans:CaptionExportSpan[]=[];
  let previousKey='',instructions=0;
  for(let frame=0;frame<frames;frame++) {
    const scene=sample(frame*1000/fps);
    if(scene) {
      const background=scene.background.opacity>0?{...scene.background,opacity:Math.min(1,scene.background.opacity)}:null;
      const words=scene.words.filter(word=>word.target.opacity>0).map(({text,target})=>{
        const tile=raster(text);
        return {...target,tile:tile.tile,x:target.x-tile.margin,y:target.y-tile.margin,
          width:tile.width,height:tile.height,opacity:Math.min(1,target.opacity)};
      });
      if(background||words.length) {
        const key=JSON.stringify([background,words]),previous=spans[spans.length-1];
        if(previous&&previous.end_frame===frame&&key===previousKey)previous.end_frame=frame+1;
        else {
          instructions+=words.length+(background?1:0);
          if(instructions>1_000_000)throw new Error('Caption animation exceeds the export memory limit');
          spans.push({start_frame:frame,end_frame:frame+1,background,words});previousKey=key;
        }
      }
    }
    if((frame+1)%240===0)await yieldControl();
  }
  return {frames,spans};
}
