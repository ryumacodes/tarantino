import type {CaptionSettings} from './settings';
import type {TextMeasurement} from './groups';
export const CAPTION_FONTS={
  'sans-serif':'-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  serif:'Georgia, "Times New Roman", serif',mono:'Menlo, Consolas, monospace',
};
export interface CaptionRaster {canvas:HTMLCanvasElement;width:number;height:number;margin:number}
/** Glyphs are cached at twice output resolution and reused across animation frames. */
export class CaptionTextAtlas {
  private context:CanvasRenderingContext2D;
  private metrics=new Map<string,TextMeasurement>();
  private readonly metricLimit=4096;
  private rasters=new Map<string,CaptionRaster>();
  private bytes=0;
  private readonly limit=32*1024*1024;
  constructor(private settings:CaptionSettings,private fontSize:number) {
    const context=document.createElement('canvas').getContext('2d');
    if(!context)throw new Error('Could not measure captions');
    this.context=context;context.font=`${fontSize}px ${CAPTION_FONTS[settings.font]}`;
  }
  measure=(text:string):TextMeasurement=>{
    const cached=this.metrics.get(text);
    if(cached){this.metrics.delete(text);this.metrics.set(text,cached);return cached;}
    const metrics=this.context.measureText(text);
    const height=(metrics.fontBoundingBoxAscent??this.fontSize*.8)+(metrics.fontBoundingBoxDescent??this.fontSize*.2);
    const size={width:metrics.width,height};
    if(this.metrics.size>=this.metricLimit)this.metrics.delete(this.metrics.keys().next().value!);
    this.metrics.set(text,size);return size;
  };
  get(text:string):CaptionRaster {
    const existing=this.rasters.get(text);
    if(existing){this.rasters.delete(text);this.rasters.set(text,existing);return existing;}
    const size=this.measure(text),metrics=this.context.measureText(text);
    const margin=Math.ceil(Math.max(2,metrics.actualBoundingBoxLeft,metrics.actualBoundingBoxRight-size.width));
    const canvas=document.createElement('canvas');
    const width=Math.ceil((size.width+margin*2)*2),height=Math.ceil((size.height+margin*2)*2);
    const bytes=Math.max(1,width)*Math.max(1,height)*4;
    if(width>16384||height>16384||bytes>this.limit)throw new Error('Caption text exceeds the raster size limit');
    while(this.bytes+bytes>this.limit&&this.rasters.size) {
      const key=this.rasters.keys().next().value!,old=this.rasters.get(key)!;
      this.bytes-=old.canvas.width*old.canvas.height*4;this.rasters.delete(key);
    }
    canvas.width=Math.max(1,width);canvas.height=Math.max(1,height);
    const ctx=canvas.getContext('2d');if(!ctx)throw new Error('Could not rasterize captions');
    ctx.scale(2,2);ctx.font=this.context.font;ctx.fillStyle=this.settings.color;ctx.textBaseline='alphabetic';
    ctx.fillText(text,margin,margin+(metrics.fontBoundingBoxAscent??this.fontSize*.8));
    const raster={canvas,...size,margin};this.rasters.set(text,raster);this.bytes+=bytes;return raster;
  }
  get rasterBytes(){return this.bytes;}
  get rasterCount(){return this.rasters.size;}
  get metricCount(){return this.metrics.size;}
}
