import { GLSL3, LinearFilter, LinearMipmapLinearFilter, NoBlending, NoColorSpace, SRGBColorSpace,
  Matrix3, RawShaderMaterial, Uniform, Vector2, Vector4, WebGLRenderTarget, type Texture, type WebGLRenderer } from 'three';
import { FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js';

const vertexShader = `precision highp float;
in vec3 position;
void main() { gl_Position=vec4(position,1.0); }`;
const fragmentShader = `precision highp float;
uniform sampler2D source;
uniform int level;
uniform bool premultiply;
uniform bool transformSource, encodeSource;
uniform mat3 sourceTransform;
uniform vec2 outputSize;
out vec4 color;
vec4 readPixel(ivec2 p) {
  return texelFetch(source,clamp(p,ivec2(0),textureSize(source,0)-1),0);
}
void main() {
  ivec2 pixel=ivec2(gl_FragCoord.xy);
  if(level==0) {
    color=transformSource ? textureLod(source,(sourceTransform*vec3(gl_FragCoord.xy/outputSize,1.0)).xy,0.0) : readPixel(pixel);
    if(encodeSource)color.rgb=mix(1.055*pow(max(color.rgb,vec3(0.0)),vec3(1.0/2.4))-0.055,
      12.92*color.rgb,lessThanEqual(color.rgb,vec3(0.0031308)));
    if(premultiply)color.rgb*=color.a; return;
  }
  // Work in top-left coordinates to agree with native odd-height mip crops.
  ivec2 sourceSize=textureSize(source,0);
  ivec2 targetSize=max(sourceSize/2,ivec2(1));
  ivec2 topPixel=ivec2(pixel.x,targetSize.y-1-pixel.y);
  ivec2 origin=topPixel*2;
  color=vec4(0.0);
  for(int y=0;y<4;y++) for(int x=0;x<4;x++) {
    if(level==1 && (x>=2 || y>=2)) continue;
    float wx=level==1 ? 0.5 : ((x==0 || x==3) ? 0.125 : 0.375);
    float wy=level==1 ? 0.5 : ((y==0 || y==3) ? 0.125 : 0.375);
    ivec2 p=origin+ivec2(x,y)-(level==1 ? ivec2(0) : ivec2(1));
    p=clamp(p,ivec2(0),sourceSize-1);
    color+=readPixel(ivec2(p.x,sourceSize.y-1-p.y))*wx*wy;
  }
}`;

/** Explicit box/binomial mip chain. Separate render targets avoid WebGL
 * feedback loops between sampled and attached levels of the same texture. */
export class LoupePyramid {
  readonly target: WebGLRenderTarget;
  readonly levels: WebGLRenderTarget[];
  private readonly material = new RawShaderMaterial({glslVersion:GLSL3,vertexShader,fragmentShader,
    depthTest:false,depthWrite:false,blending:NoBlending,toneMapped:false,
    uniforms:{source:new Uniform(null),level:new Uniform(0),premultiply:new Uniform(false),
      transformSource:new Uniform(false),encodeSource:new Uniform(false),sourceTransform:new Uniform(new Matrix3()),outputSize:new Uniform(new Vector2())}});
  private readonly quad = new FullScreenQuad(this.material);

  constructor(readonly width:number, readonly height:number, premultiply=false) {
    this.material.uniforms.premultiply.value=premultiply;
    const count=Math.floor(Math.log2(Math.max(width,height)))+1;
    const options={depthBuffer:false,stencilBuffer:false,generateMipmaps:false,
      minFilter:LinearFilter,magFilter:LinearFilter,colorSpace:NoColorSpace};
    this.target=new WebGLRenderTarget(width,height,{...options,minFilter:LinearMipmapLinearFilter});
    this.levels=Array.from({length:count},(_,level)=>level===0?this.target:new WebGLRenderTarget(
      Math.max(1,width>>level),Math.max(1,height>>level),options));
    this.target.texture.mipmaps=this.levels.map(level=>({width:level.width,height:level.height,data:new Uint8Array(0)}));
  }

  update(renderer:WebGLRenderer,source:Texture,transform?:Matrix3,lastLevel=this.levels.length-1) {
    this.material.uniforms.transformSource.value=!!transform;
    this.material.uniforms.encodeSource.value=!!transform&&source.colorSpace===SRGBColorSpace
      &&!('isVideoTexture' in source&&source.isVideoTexture);
    if(transform)this.material.uniforms.sourceTransform.value.copy(transform);
    this.material.uniforms.outputSize.value.set(this.width,this.height);
    const previous=renderer.getRenderTarget();
    const face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4());
    const scissor=renderer.getScissor(new Vector4()),scissorTest=renderer.getScissorTest();
    const autoClear=renderer.autoClear;
    try {
      renderer.autoClear=false;
      renderer.initRenderTarget(this.target);
      for(let i=0;i<this.levels.length && i<=lastLevel;i++) {
        const level=this.levels[i];
        this.material.uniforms.source.value=i===0 ? source : this.levels[i-1].texture;
        this.material.uniforms.level.value=i;
        renderer.setRenderTarget(level);
        renderer.setScissorTest(false);
        this.quad.render(renderer);
        if(i>0)renderer.copyTextureToTexture(level.texture,this.target.texture,null,null,0,i);
      }
    } finally {
      renderer.setRenderTarget(previous,face,mip);
      renderer.setViewport(viewport);
      renderer.setScissor(scissor);
      renderer.setScissorTest(scissorTest);
      renderer.autoClear=autoClear;
    }
  }

  dispose() {
    this.levels.forEach(level=>level.dispose());
    this.material.dispose();this.quad.dispose();
  }
}
