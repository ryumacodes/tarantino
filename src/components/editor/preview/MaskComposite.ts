import {GLSL3,NoBlending,RawShaderMaterial,Uniform,Vector4,type Texture,type WebGLRenderTarget,type WebGLRenderer} from 'three';
import {FullScreenQuad} from 'three/examples/jsm/postprocessing/Pass.js';
import {captionCorner} from '../../../utils/captions/corners';
export interface MaskPlacement {
  rect:[number,number,number,number];radius:number;opacity:number;highlightOpacity:number;sensitive:boolean;
}
const vertexShader=`precision highp float;
in vec3 position;
void main(){gl_Position=vec4(position,1.0);}`;
const fragmentShader=`precision highp float;
uniform sampler2D scene;
uniform sampler2D colors;
uniform vec4 rect;
uniform vec4 controls;
uniform float curvature[16];
uniform bool encodedStraight;
out vec4 color;
float radialRatio(vec2 q){
  if(q.x<=0.0||q.y<=0.0)return 1.0;
  float sampleIndex=clamp(atan(min(q.x,q.y),max(q.x,q.y))*19.098593,0.0,15.0);
  int low=int(sampleIndex),high=min(low+1,15);
  return mix(curvature[low],curvature[high],fract(sampleIndex));
}
void main(){
  vec4 inputColor=texelFetch(scene,ivec2(gl_FragCoord.xy),0);
  if(controls.y==0.0){color=inputColor;return;}
  vec2 position=vec2(gl_FragCoord.x,float(textureSize(scene,0).y)-gl_FragCoord.y);
  vec2 halfSize=rect.zw*0.5;
  vec2 q=abs(position-rect.xy-halfSize)-halfSize+controls.x;
  float distance=length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-controls.x*radialRatio(q);
  float aa=max(fwidth(distance),0.00001);
  float inside=1.0-smoothstep(-aa,aa,distance);
  vec2 uv=clamp((position-rect.xy)/max(rect.zw,vec2(0.000001)),vec2(0.0),vec2(1.0))*0.5+0.25;
  vec4 texel=textureLod(colors,vec2(uv.x,1.0-uv.y),0.0);
  vec3 straight=texel.rgb/max(texel.a,0.00001);
  vec4 linear=encodedStraight?vec4(inputColor.rgb*inputColor.rgb*inputColor.a,inputColor.a):inputColor;
  vec4 covered=mix(linear,vec4(straight*straight,1.0),inside*controls.y);
  vec4 dimmed=mix(linear,vec4(0.0,0.0,0.0,1.0),controls.z*(1.0-inside)*controls.y);
  vec4 result=mix(dimmed,covered,controls.w);
  if(all(equal(result,linear))){color=inputColor;return;}
  color=encodedStraight?vec4(sqrt(max(result.rgb,vec3(0.0))/max(result.a,0.00001)),result.a):result;
}`;
/** Apply a mask in source-pixel coordinates before scene framing and zoom. */
export class MaskComposite {
  private readonly material:RawShaderMaterial;
  private readonly quad:FullScreenQuad;
  private cornerKey='';
  constructor(encodedStraight=true){
    this.material=new RawShaderMaterial({glslVersion:GLSL3,vertexShader,fragmentShader,depthTest:false,depthWrite:false,
      blending:NoBlending,toneMapped:false,uniforms:{scene:new Uniform(null),colors:new Uniform(null),
        rect:new Uniform(new Vector4()),controls:new Uniform(new Vector4()),curvature:new Uniform(new Float32Array(16)),
        encodedStraight:new Uniform(encodedStraight)}});
    this.quad=new FullScreenQuad(this.material);
  }
  update(renderer:WebGLRenderer,scene:Texture,colors:Texture,target:WebGLRenderTarget,placement:MaskPlacement){
    const {rect,radius,opacity,highlightOpacity,sensitive}=placement;
    if(![...rect,radius,opacity,highlightOpacity].every(Number.isFinite)||rect[2]<=0||rect[3]<=0
      ||radius<0||opacity<0||opacity>1||highlightOpacity<0||highlightOpacity>1)
      throw new RangeError('Invalid mask placement');
    if(scene===target.texture||colors===target.texture)throw new Error('Mask output must use a separate texture');
    const uniforms=this.material.uniforms,key=`${rect[2]}:${rect[3]}:${radius}`;
    if(key!==this.cornerKey){
      const corner=captionCorner(rect[2],rect[3],radius);
      uniforms.curvature.value.set(corner.curvature);uniforms.controls.value.x=corner.radius;this.cornerKey=key;
    }
    uniforms.scene.value=scene;uniforms.colors.value=colors;uniforms.rect.value.set(...rect);
    uniforms.controls.value.y=opacity;uniforms.controls.value.z=highlightOpacity;uniforms.controls.value.w=sensitive?1:0;
    const previous=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4());
    const scissorTest=renderer.getScissorTest(),autoClear=renderer.autoClear;
    try{renderer.autoClear=false;renderer.setRenderTarget(target);renderer.setScissorTest(false);this.quad.render(renderer);}
    finally{renderer.setRenderTarget(previous,face,mip);renderer.setViewport(viewport);renderer.setScissor(scissor);
      renderer.setScissorTest(scissorTest);renderer.autoClear=autoClear;}
  }
  dispose(){this.material.dispose();this.quad.dispose();}
}
