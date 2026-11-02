import type {ShortcutSettings} from './settings';
import type {AnimatedKeycap} from './keycapMotion';
import {drawKeycaps,shortcutLayout} from './keycaps';
import {keycapDraw,type KeycapAtlasEntry} from './animatedExport';
export interface KeycapRaster {entry:KeycapAtlasEntry;canvas:HTMLCanvasElement}

/** One raster per label and output geometry, shared by preview and export. */
export class KeycapAtlas {
  readonly rasters:KeycapRaster[]=[];
  private labels=new Map<string,KeycapRaster>();
  constructor(private settings:ShortcutSettings,private width:number,private height:number) {}
  get(label:string):KeycapRaster {
    const existing=this.labels.get(label);if(existing)return existing;
    const base=shortcutLayout([label],this.settings,this.width,this.height)[0];
    const margin=Math.ceil(base.height*0.7);
    const scale=Math.min(1,this.width/(base.width+2*margin),this.height/(base.height+2*margin));
    const contentX=margin*scale,contentY=margin*scale;
    const contentWidth=base.width*scale,contentHeight=base.height*scale;
    const canvas=document.createElement('canvas');
    canvas.width=Math.min(this.width,Math.ceil((base.width+2*margin)*scale));
    canvas.height=Math.min(this.height,Math.ceil((base.height+2*margin)*scale));
    const ctx=canvas.getContext('2d');if(!ctx)throw new Error('Could not render shortcut keycaps');
    drawKeycaps(ctx,[{label,x:contentX,y:contentY,width:contentWidth,height:contentHeight}]);
    const entry={tile:this.rasters.length,width:canvas.width,height:canvas.height,contentX,contentY,contentWidth,contentHeight};
    const raster={entry,canvas};this.labels.set(label,raster);this.rasters.push(raster);
    return raster;
  }
  draw(ctx:CanvasRenderingContext2D,caps:readonly AnimatedKeycap[]) {
    ctx.save();ctx.imageSmoothingEnabled=true;ctx.imageSmoothingQuality='low';
    for(const cap of caps) {
      if(cap.opacity<=0)continue;
      const raster=this.get(cap.label),draw=keycapDraw(cap,raster.entry);
      ctx.globalAlpha=draw.opacity;
      ctx.drawImage(raster.canvas,draw.x,draw.y,draw.width,draw.height);
    }
    ctx.restore();
  }
}
