import {LinearFilter,NoColorSpace,WebGLRenderTarget,type Texture,type WebGLRenderer} from 'three';
import {maskCoverSampling} from '../../../utils/masks/coverSampling';
import {LoupePyramid} from './LoupePyramid';
import {MaskCover} from './MaskCover';
import {MaskComposite,type MaskPlacement} from './MaskComposite';
/** Input is an already cropped, encoded-straight source texture. Keep this stage
 * ahead of zoom/framing so coverage moves with the recording, not the canvas. */
export class MaskStage {
  private readonly pyramid:LoupePyramid;
  private readonly cover=new MaskCover();
  private readonly composite:MaskComposite;
  readonly target:WebGLRenderTarget;
  constructor(readonly width:number,readonly height:number){
    this.composite=new MaskComposite(true);
    if(!Number.isInteger(width)||!Number.isInteger(height)||width<=0||height<=0)
      throw new RangeError('Invalid mask source dimensions');
    this.pyramid=new LoupePyramid(width,height,true);
    this.target=new WebGLRenderTarget(width,height,{depthBuffer:false,stencilBuffer:false,
      minFilter:LinearFilter,magFilter:LinearFilter,generateMipmaps:false,colorSpace:NoColorSpace});
  }
  update(renderer:WebGLRenderer,source:Texture,placement:MaskPlacement|null):Texture{
    if(!placement||placement.opacity===0)return source;
    const [x,y,width,height]=placement.rect;
    const plan=maskCoverSampling({x:x/this.width,y:y/this.height,width:width/this.width,height:height/this.height},
      {x:0,y:0,width:1,height:1},{width:this.width,height:this.height},this.pyramid.levels.length);
    if(plan){
      this.pyramid.update(renderer,source);
      this.cover.update(renderer,this.pyramid.target.texture,plan);
    }
    this.composite.update(renderer,source,this.cover.target.texture,this.target,placement);
    return this.target.texture;
  }
  dispose(){this.pyramid.dispose();this.cover.dispose();this.composite.dispose();this.target.dispose();}
}
