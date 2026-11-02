import {timelineDuration} from '../timeline/previewTimeline';
import {createCaptionRenderer,type CaptionRenderState} from './renderer';
import {captionExportPlan,type CaptionExportTile} from './exportPlan';

/** Native export preparation. Encode each distinct word once, independent of
 * frame count. The consumer renders the background as a rounded rectangle. */
export async function captionExportPayload(state:CaptionRenderState,width:number,height:number,fps:number) {
  if(!state.captionSettings.enabled||!state.transcripts.some(session=>session.words.length))return undefined;
  await document.fonts.ready;
  const renderer=createCaptionRenderer(state,width,height,fps);
  const duration=Math.max(0,Math.min(state.trimEnd,timelineDuration(state))-state.trimStart);
  const frames=Math.max(1,Math.ceil(duration*fps/1000));
  const entries=new Map<string,CaptionExportTile>(),tiles:{png:string}[]=[];
  let rasterBytes=0,encodedBytes=0;
  const plan=await captionExportPlan(frames,fps,renderer.sampleScene,text=>{
    const existing=entries.get(text);if(existing)return existing;
    const raster=renderer.atlas.get(text),canvas=raster.canvas;
    rasterBytes+=canvas.width*canvas.height*4;
    if(rasterBytes>128*1024*1024)throw new Error('Caption images exceed the export memory limit');
    const png=canvas.toDataURL('image/png').split(',')[1];
    encodedBytes+=png.length;
    if(encodedBytes>128*1024*1024)throw new Error('Encoded captions exceed the export memory limit');
    const entry={tile:tiles.length,width:canvas.width/2,height:canvas.height/2,margin:raster.margin};
    tiles.push({png});entries.set(text,entry);return entry;
  });
  if(!plan.spans.length)return undefined;
  return {width,height,frames,tiles,spans:plan.spans,backgroundColor:state.captionSettings.backgroundColor,cornerRadius:12};
}
