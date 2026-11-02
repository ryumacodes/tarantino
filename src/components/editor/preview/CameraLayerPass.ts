import { BlendFunction, Effect, EffectPass } from 'postprocessing';
import { Camera, Texture, Uniform, UnsignedByteType, Vector2, Vector4, WebGLRenderTarget, type WebGLRenderer } from 'three';

export interface CameraLayerFrame {
  texture: Texture;
  sourceWidth: number;
  sourceHeight: number;
  /** Top-left output pixels, then width and height. */
  rect: readonly [number, number, number, number];
  shape: 'circle' | 'roundrect' | 'fullscreen';
  opacity: number;
  mirrored?: boolean;
}

const shader = `
uniform sampler2D cameraTexture;
uniform vec2 cameraSourceSize;
uniform vec2 outputSize;
uniform vec4 cameraRect;
uniform float cameraShape;
uniform float cameraOpacity;
uniform float cameraMirrored;
uniform float premultipliedInput;
void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  outputColor=inputColor;
  vec2 pixel=vec2(floor(uv.x*outputSize.x),outputSize.y-1.0-floor(uv.y*outputSize.y));
  vec2 local=pixel-cameraRect.xy;
  if(cameraOpacity<=0.0 || any(lessThan(local,vec2(0.0))) || any(greaterThanEqual(local,cameraRect.zw))) return;
  vec2 cameraUV=local/cameraRect.zw;
  vec2 sampleUV=cameraUV;
  float aspect=(cameraSourceSize.x/max(cameraSourceSize.y,1.0))/(cameraRect.z/cameraRect.w);
  if(aspect>1.0) sampleUV.x=0.5+(cameraUV.x-0.5)/aspect;
  else sampleUV.y=0.5+(cameraUV.y-0.5)*aspect;
  if(cameraMirrored>0.5) sampleUV.x=1.0-sampleUV.x;
  vec4 color=texture2D(cameraTexture,vec2(sampleUV.x,1.0-sampleUV.y));
  float mask=1.0;
  if(cameraShape<0.5) mask=1.0-smoothstep(0.49,0.5,length(cameraUV-vec2(0.5)));
  else if(cameraShape<1.5) {
    float radius=min(cameraRect.z*0.15,min(cameraRect.z,cameraRect.w)*0.5);
    vec2 q=abs(local-cameraRect.zw*0.5)-(cameraRect.zw*0.5-vec2(radius));
    float distance=length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-radius;
    mask=1.0-smoothstep(-0.5,0.5,distance);
  }
  float alpha=color.a*mask*cameraOpacity;
  if(alpha<=0.001) return;
  float combined=alpha+inputColor.a*(1.0-alpha);
  vec3 base=mix(inputColor.rgb*inputColor.a,inputColor.rgb,premultipliedInput);
  vec3 rgb=color.rgb*alpha+base*(1.0-alpha);
  outputColor=vec4(premultipliedInput>0.5 ? rgb : rgb/max(combined,0.00001),combined);
}`;

export class CameraLayerEffect extends Effect {
  constructor() {
    super('CameraLayer',shader,{blendFunction:BlendFunction.SRC,uniforms:new Map<string,Uniform>([
      ['cameraTexture',new Uniform(null)],['cameraSourceSize',new Uniform(new Vector2(1,1))],
      ['outputSize',new Uniform(new Vector2(1,1))],['cameraRect',new Uniform(new Vector4())],
      ['cameraMirrored',new Uniform(0)],['cameraShape',new Uniform(0)],['cameraOpacity',new Uniform(0)],['premultipliedInput',new Uniform(1)],
    ])});
  }
  setFrame(width:number,height:number,frame:CameraLayerFrame|null,premultiplied:boolean) {
    const u=this.uniforms;
    u.get('cameraOpacity')!.value=frame && Number.isFinite(frame.opacity)?Math.max(0,Math.min(1,frame.opacity)):0;
    if(!frame)return;
    u.get('cameraTexture')!.value=frame.texture;
    u.get('cameraMirrored')!.value=Number(frame.mirrored ?? false);
    u.get('cameraSourceSize')!.value.set(frame.sourceWidth,frame.sourceHeight);
    u.get('outputSize')!.value.set(width,height);
    u.get('cameraRect')!.value.fromArray(frame.rect);
    u.get('cameraShape')!.value=frame.shape==='circle'?0:frame.shape==='roundrect'?1:2;
    u.get('premultipliedInput')!.value=Number(premultiplied);
  }
}

/** GPU camera overlay reusable on the visible stage and premultiplied lens foreground. */
export class CameraLayerPass {
  readonly target: WebGLRenderTarget;
  private readonly effect=new CameraLayerEffect();
  private readonly pass: EffectPass;
  private initialized=false;
  constructor(width:number,height:number) {
    this.pass=new EffectPass(new Camera(),this.effect);
    this.target=new WebGLRenderTarget(width,height,{depthBuffer:false});
    this.pass.setSize(width,height);
  }
  render(renderer:WebGLRenderer,input:WebGLRenderTarget,frame:CameraLayerFrame,premultiplied=true) {
    if(!this.initialized){this.pass.initialize(renderer,false,UnsignedByteType);this.initialized=true;}
    if(this.target.width!==input.width || this.target.height!==input.height){
      this.target.setSize(input.width,input.height);this.pass.setSize(input.width,input.height);
    }
    this.effect.setFrame(input.width,input.height,frame,premultiplied);
    const target=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4()),test=renderer.getScissorTest();
    try {renderer.setScissorTest(false);this.pass.render(renderer,input,this.target,0,false);}
    finally {renderer.setRenderTarget(target,face,mip);renderer.setViewport(viewport);renderer.setScissor(scissor);renderer.setScissorTest(test);}
    return this.target.texture;
  }
  dispose(){this.pass.dispose();this.target.dispose();}
}
