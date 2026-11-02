import {
  Camera, GLSL3, Matrix3, Mesh, NoBlending, PlaneGeometry, RawShaderMaterial,
  Scene, Texture, Vector2, Vector4, WebGLRenderer, WebGLRenderTarget,
} from 'three';
import { temporalWindowShader } from './shaders/temporalWindow.glsl';

export interface TemporalWindowInputs {
  source: Texture;
  mask: Texture;
  shadow: Texture;
  /** All rectangles/positions are in top-left output pixels; rectangles are center + size. */
  before: readonly [number,number,number,number];
  now: readonly [number,number,number,number];
  inputSize: readonly [number,number];
  baseCenter: readonly [number,number];
  sourceSize: readonly [number,number];
  cornerRadius: number;
  hasNativeMask: boolean;
  shadowEnabled: boolean;
  sourceTransform?: Matrix3;
  maskTransform?: Matrix3;
}

/** Renders a premultiplied moving window/shadow layer. Input textures are borrowed. */
export class TemporalWindowPass {
  readonly target: WebGLRenderTarget;
  private readonly scene = new Scene();
  private readonly camera = new Camera();
  private readonly geometry = new PlaneGeometry(2,2);
  private readonly material = new RawShaderMaterial({
    glslVersion: GLSL3, blending: NoBlending, depthTest: false, depthWrite: false,
    vertexShader: 'precision highp float; in vec3 position; void main(){gl_Position=vec4(position,1.0);}',
    fragmentShader: temporalWindowShader,
    uniforms: {
      sourceTexture:{value:null},maskTexture:{value:null},shadowTexture:{value:null},
      sourceTransform:{value:new Matrix3()},maskTransform:{value:new Matrix3()},
      outputSize:{value:new Vector2()},inputSize:{value:new Vector2()},baseCenter:{value:new Vector2()},sourceSize:{value:new Vector2()},
      shutterStart:{value:new Vector4()},shutterNow:{value:new Vector4()},
      cornerRadius:{value:0},hasNativeMask:{value:0},shadowEnabled:{value:0},
    },
  });

  constructor(width: number,height: number) {
    this.target=new WebGLRenderTarget(width,height,{depthBuffer:false});
    this.target.texture.name='Temporal window foreground';
    this.scene.add(new Mesh(this.geometry,this.material));
  }

  render(renderer: WebGLRenderer,width: number,height: number,input: TemporalWindowInputs) {
    this.target.setSize(width,height);
    const u=this.material.uniforms;
    u.sourceTexture.value=input.source;u.maskTexture.value=input.mask;u.shadowTexture.value=input.shadow;
    u.outputSize.value.set(width,height);u.inputSize.value.set(...input.inputSize);
    u.baseCenter.value.set(...input.baseCenter);u.sourceSize.value.set(...input.sourceSize);
    u.shutterStart.value.set(...input.before);u.shutterNow.value.set(...input.now);
    u.cornerRadius.value=input.cornerRadius;u.hasNativeMask.value=Number(input.hasNativeMask);u.shadowEnabled.value=Number(input.shadowEnabled);
    u.sourceTransform.value.identity();u.maskTransform.value.identity();
    if(input.sourceTransform) u.sourceTransform.value.copy(input.sourceTransform);
    if(input.maskTransform) u.maskTransform.value.copy(input.maskTransform);
    const target=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4());
    const scissorTest=renderer.getScissorTest(),autoClear=renderer.autoClear;
    try {
      renderer.autoClear=false;renderer.setScissorTest(false);renderer.setRenderTarget(this.target);
      renderer.setViewport(0,0,width,height);renderer.render(this.scene,this.camera);
    } finally {
      renderer.autoClear=autoClear;renderer.setRenderTarget(target,face,mip);
      renderer.setViewport(viewport);renderer.setScissor(scissor);renderer.setScissorTest(scissorTest);
    }
    return this.target.texture;
  }

  dispose() {this.geometry.dispose();this.material.dispose();this.target.dispose();}
}
