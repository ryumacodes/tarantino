import {GLSL3,LinearFilter,NoBlending,NoColorSpace,SRGBColorSpace,RawShaderMaterial,Uniform,Vector4,
 WebGLRenderTarget,type Texture,type WebGLRenderer} from 'three';
import {FullScreenQuad} from 'three/examples/jsm/postprocessing/Pass.js';
import type {PixelCrop} from '../../../utils/sourceCrop';
/** Extract the decoder-equivalent pixel crop without applying animated view UVs. */
export class MaskSource {
 readonly target:WebGLRenderTarget;
 private readonly material=new RawShaderMaterial({glslVersion:GLSL3,depthTest:false,depthWrite:false,
  blending:NoBlending,toneMapped:false,uniforms:{source:new Uniform(null),crop:new Uniform(new Vector4()),dimensions:new Uniform(new Vector4()),encode:new Uniform(false)},
  vertexShader:`precision highp float;in vec3 position;void main(){gl_Position=vec4(position,1.0);}`,
  fragmentShader:`precision highp float;uniform sampler2D source;uniform vec4 crop;uniform vec4 dimensions;uniform bool encode;out vec4 color;
  void main(){vec2 position=vec2(gl_FragCoord.x/dimensions.z,1.0-gl_FragCoord.y/dimensions.w)*crop.zw+crop.xy;
   vec2 uv=position/dimensions.xy;color=texture(source,vec2(uv.x,1.0-uv.y));
   if(encode)color.rgb=mix(1.055*pow(max(color.rgb,vec3(0.0)),vec3(1.0/2.4))-0.055,
    12.92*color.rgb,lessThanEqual(color.rgb,vec3(0.0031308)));}`});
 private readonly quad=new FullScreenQuad(this.material);
 constructor(readonly crop:PixelCrop,width=crop.width,height=crop.height){
  if(![width,height,crop.sourceWidth,crop.sourceHeight].every(value=>Number.isInteger(value)&&value>0))
   throw new RangeError('Invalid mask source dimensions');
  this.target=new WebGLRenderTarget(width,height,{depthBuffer:false,stencilBuffer:false,
   minFilter:LinearFilter,magFilter:LinearFilter,colorSpace:NoColorSpace,generateMipmaps:false});
 }
 update(renderer:WebGLRenderer,source:Texture){
  this.material.uniforms.source.value=source;
  this.material.uniforms.crop.value.set(this.crop.x,this.crop.y,this.crop.width,this.crop.height);
  this.material.uniforms.dimensions.value.set(this.crop.sourceWidth,this.crop.sourceHeight,this.target.width,this.target.height);
  // Video uploads remain encoded; ordinary sRGB textures are hardware-decoded.
  this.material.uniforms.encode.value=source.colorSpace===SRGBColorSpace&&!(source as Texture&{isVideoTexture?:boolean}).isVideoTexture;
  const target=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
  const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4());
  const scissorTest=renderer.getScissorTest(),autoClear=renderer.autoClear;
  try{renderer.autoClear=false;renderer.setRenderTarget(this.target);renderer.setScissorTest(false);this.quad.render(renderer);}
  finally{renderer.setRenderTarget(target,face,mip);renderer.setViewport(viewport);renderer.setScissor(scissor);
   renderer.setScissorTest(scissorTest);renderer.autoClear=autoClear;}
  return this.target.texture;
 }
 dispose(){this.target.dispose();this.material.dispose();this.quad.dispose();}
}
