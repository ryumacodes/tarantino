import {GLSL3, NoBlending, NoColorSpace, RawShaderMaterial, Uniform, Vector4,
  WebGLRenderTarget, type WebGLRenderer} from 'three';
import {FullScreenQuad} from 'three/examples/jsm/postprocessing/Pass.js';
import {cursorBodyShader} from './shaders/cursorBody.glsl';
import {LoupePyramid} from './LoupePyramid';

// Four texels per cursor unit keeps small outlined/dotted shapes crisp. Empty
// margins leave room for the blur kernel without repeating colored edge texels.
export const CURSOR_TILE_SIZE=256;
export const CURSOR_TILE_SCALE=4;
export const CURSOR_TILE_ORIGIN=96;
export class CursorTile {
  readonly pyramid=new LoupePyramid(CURSOR_TILE_SIZE,CURSOR_TILE_SIZE);
  private readonly source=new WebGLRenderTarget(CURSOR_TILE_SIZE,CURSOR_TILE_SIZE,
    {depthBuffer:false,stencilBuffer:false,colorSpace:NoColorSpace});
  private readonly material=new RawShaderMaterial({glslVersion:GLSL3,
    depthTest:false,depthWrite:false,blending:NoBlending,toneMapped:false,
    uniforms:{uCursorOpacity:new Uniform(1),uCursorStyle:new Uniform(0),
      uShadowIntensity:new Uniform(0),tileColor:new Uniform([1,1,1])},
    vertexShader:'precision highp float; in vec3 position; void main(){gl_Position=vec4(position,1.0);}',
    fragmentShader:`precision highp float;
      uniform float uCursorOpacity,uCursorStyle,uShadowIntensity;
      uniform vec3 tileColor;
      out vec4 tileOutput;
      ${cursorBodyShader}
      void main() {
        vec2 p=vec2(gl_FragCoord.x,${CURSOR_TILE_SIZE}.0-gl_FragCoord.y);
        vec4 color=cursorBody(p,vec4(${CURSOR_TILE_ORIGIN}.0,${CURSOR_TILE_ORIGIN}.0,${CURSOR_TILE_SCALE}.0,0.0),tileColor);
        tileOutput=vec4(color.rgb*color.a,color.a);
      }`});
  private readonly quad=new FullScreenQuad(this.material);
  private key='';

  update(renderer:WebGLRenderer,style:number,shadow:number,color:number[]) {
    const key=[style,shadow,...color].join(',');
    if(key===this.key)return;
    const target=renderer.getRenderTarget(),face=renderer.getActiveCubeFace(),mip=renderer.getActiveMipmapLevel();
    const viewport=renderer.getViewport(new Vector4()),scissor=renderer.getScissor(new Vector4());
    const scissorTest=renderer.getScissorTest(),autoClear=renderer.autoClear;
    try {
      this.material.uniforms.uCursorStyle.value=style;
      this.material.uniforms.uShadowIntensity.value=shadow;
      this.material.uniforms.tileColor.value=color;
      renderer.autoClear=false;
      renderer.setRenderTarget(this.source);renderer.setScissorTest(false);
      this.quad.render(renderer);
      this.pyramid.update(renderer,this.source.texture);
      this.key=key;
    } finally {
      renderer.setRenderTarget(target,face,mip);renderer.setViewport(viewport);
      renderer.setScissor(scissor);renderer.setScissorTest(scissorTest);renderer.autoClear=autoClear;
    }
  }
  dispose() {this.source.dispose();this.pyramid.dispose();this.material.dispose();this.quad.dispose();}
}
