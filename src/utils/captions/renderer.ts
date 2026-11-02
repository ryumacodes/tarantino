import {previewSample} from '../timeline/previewTimeline';
import type {EditorState} from '../../stores/editor/types';
import {prepareCaptionGroups} from './prepare';
import {createCaptionBackground} from './background';
import {CaptionTextAtlas} from './raster';
import {createCaptionMotion,type PreparedCaptionSource} from './motionTimeline';
import type {CaptionSourceWord} from './prepare';
import type {CaptionFramePart} from './motion';
export interface CaptionScene {background:CaptionFramePart;words:{text:string;target:CaptionFramePart}[]}
export type CaptionRenderState=Pick<EditorState,'clips'|'tracks'|'duration'|'trimStart'|'trimEnd'|'videoFilePath'|'transcripts'|'captionSettings'>;
/** Canvas renderer for preview/fallback. Create after fonts settle and recreate
 * after edits; text rasters and prepared source groups stay cached between frames. */
export function createCaptionRenderer(state:CaptionRenderState,width:number,height:number,fps:number) {
  const settings=state.captionSettings,fontSize=Math.min(width,height)*settings.sizeRatio;
  if(!Number.isFinite(fontSize)||fontSize<=0)throw new RangeError('Invalid caption render dimensions');
  const drawBackground=createCaptionBackground(settings.backgroundColor);
  const atlas=new CaptionTextAtlas(settings,fontSize),sources=new Map<string,PreparedCaptionSource<CaptionSourceWord>>();
  const resolve=(path:string)=>{
    let source=sources.get(path);
    if(!source) {
      source={fontSize,groups:prepareCaptionGroups(state,state.videoFilePath??'',path,state.transcripts,
        settings.showSoundLabels,atlas.measure,width)};
      sources.set(path,source);
    }
    return source;
  };
  const motion=createCaptionMotion(state,state.videoFilePath??'',resolve,settings,{x:0,y:0,width,height},settings.position,fps);
  const partStyle=(ctx:CanvasRenderingContext2D,part:CaptionFramePart)=>{
    ctx.globalAlpha=Math.max(0,Math.min(1,part.opacity));
    ctx.filter=part.blurRadius>0?`blur(${part.blurRadius}px)`:'none';
  };
  const sampleScene=(time:number):CaptionScene|null=>{
    const frame=settings.enabled?motion.sample(time):null;if(!frame)return null;
    const source=motionSource(time);
    return {background:frame.background,words:source?frame.words.flatMap(word=>{
      const text=source.groups[word.groupIndex]?.words[word.wordIndex]?.text;
      return text===undefined?[]:[{text,target:word.target}];
    }):[]};
  };
  return {
    atlas,sampleScene,
    sample:(time:number)=>settings.enabled?motion.sample(time):null,
    draw(ctx:CanvasRenderingContext2D,time:number) {
      ctx.clearRect(0,0,width,height);
      const frame=sampleScene(time);if(!frame)return;
      ctx.save();
      try {
        const background=frame.background;
        if(background.opacity>0) {
          partStyle(ctx,background);drawBackground(ctx,background.x,background.y,background.width,background.height);
        }
        for(const word of frame.words) {
          const part=word.target;if(part.opacity<=0)continue;
          const text=word.text;
          const raster=atlas.get(text);partStyle(ctx,part);
          ctx.drawImage(raster.canvas,part.x-raster.margin,part.y-raster.margin,raster.canvas.width/2,raster.canvas.height/2);
        }
      } finally {ctx.restore();}
    },
  };
  function motionSource(time:number) {
    const sample=previewSample(state,state.trimStart+time);
    return sample.sourceTime===null?undefined:resolve(sample.sourceFilePath??state.videoFilePath??'');
  }
}
