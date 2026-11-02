import {GLSL3,LinearFilter,NoBlending,NoColorSpace,RawShaderMaterial,Uniform,Vector2,Vector4,
  WebGLRenderTarget,type Texture,type WebGLRenderer} from 'three';
import {FullScreenQuad} from 'three/examples/jsm/postprocessing/Pass.js';
import type {MaskCoverSampling} from '../../../utils/masks/coverSampling';
const vertexShader=`precision highp float;
in vec3 position;
void main(){gl_Position=vec4(position,1.0);}`;
const fragmentShader=`precision highp float;
uniform sampler2D source;
uniform vec4 lattice;
uniform vec2 span;
uniform ivec2 taps;
uniform float level;
out vec4 color;
void main(){
  // Geometry and native sampling use a top-left origin; WebGL targets use bottom-left.
  vec2 quadrant=vec2(floor(gl_FragCoord.x),1.0-floor(gl_FragCoord.y));
  vec2 extent=lattice.zw*0.5;
  vec2 origin=lattice.xy+quadrant*extent;
  vec2 stepSize=extent/vec2(taps);
  color=vec4(0.0);
  for(int y=0;y<taps.y;y++)for(int x=0;x<taps.x;x++){
    vec2 uv=(origin+(vec2(x,y)+0.5)*stepSize)/span;
    color+=textureLod(source,vec2(uv.x,1.0-uv.y),level);
  }
  color/=float(taps.x*taps.y);
}`;
/** Bounded GPU quadrant summary shared with native mask composition. Input is
 * premultiplied encoded RGBA; no CPU readback or per-frame texture allocation. */
export class MaskCover {
  readonly target=new WebGLRenderTarget(2,2,{depthBuffer:false,stencilBuffer:false,
    minFilter:LinearFilter,magFilter:LinearFilter,generateMipmaps:false,colorSpace:NoColorSpace});
  private readonly material=new RawShaderMaterial({glslVersion:GLSL3,vertexShader,fragmentShader,
    depthTest:false,depthWrite:false,blending:NoBlending,toneMapped:false,
    uniforms:{source:new Uniform(null),lattice:new Uniform(new Vector4()),span:new Uniform(new Vector2()),
      taps:new Uniform(new Vector2()),level:new Uniform(0)}});
  private readonly quad=new FullScreenQuad(this.material);
  update(renderer:WebGLRenderer,source:Texture,plan:MaskCoverSampling){
    const {lattice,levelSpan,tapsPerQuadrant:taps,level}=plan;
    if(![lattice.x,lattice.y,lattice.width,lattice.height,levelSpan.width,levelSpan.height].every(Number.isFinite)
      ||lattice.width<0||lattice.height<0||levelSpan.width<=0||levelSpan.height<=0
      ||!Number.isInteger(level)||level<0||level>31||!Number.isInteger(taps.x)||!Number.isInteger(taps.y)
      ||taps.x<1||taps.y<1||taps.x*taps.y>256)throw new RangeError('Invalid mask cover sampling plan');
    const uniforms=this.material.uniforms;
    uniforms.source.value=source;uniforms.lattice.value.set(lattice.x,lattice.y,lattice.width,lattice.height);
    uniforms.span.value.set(levelSpan.width,levelSpan.height);uniforms.taps.value.set(taps.x,taps.y);uniforms.level.value=level;
    const previous=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4());
    const scissorTest=renderer.getScissorTest(),autoClear=renderer.autoClear;
    try{renderer.autoClear=false;renderer.setRenderTarget(this.target);renderer.setScissorTest(false);this.quad.render(renderer);}
    finally{renderer.setRenderTarget(previous,face,mip);renderer.setViewport(viewport);renderer.setScissor(scissor);
      renderer.setScissorTest(scissorTest);renderer.autoClear=autoClear;}
  }
  dispose(){this.target.dispose();this.material.dispose();this.quad.dispose();}
}
