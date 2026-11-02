import {mipBlurShader} from '../../components/editor/preview/shaders/mipBlur.glsl';
import {loupeNoise} from '../../components/editor/preview/loupeNoise';
import {KeycapAtlas} from './keycapAtlas';
import {keycapMipmaps,bottomUpPixels} from './keycapMipmaps';
import {keycapDraw} from './animatedExport';
import type {AnimatedKeycap} from './keycapMotion';
const vertex=`#version 300 es
uniform vec2 resolution;
uniform vec4 rect;
out vec2 uv;
void main(){uv=vec2(float(gl_VertexID%2),float(gl_VertexID/2));
vec2 p=(rect.xy+uv*rect.zw)/resolution;
gl_Position=vec4(p.x*2.0-1.0,1.0-p.y*2.0,0.0,1.0);}`;
const fragment=`#version 300 es
precision highp float;
uniform sampler2D source;
uniform sampler2D noiseTile;
uniform vec2 resolution;
uniform vec2 size;
uniform vec4 rect;
uniform float opacity;
uniform float blurRadius;
in vec2 uv;
out vec4 color;
${mipBlurShader}
void main(){
vec2 pixel=vec2(gl_FragCoord.x,resolution.y-gl_FragCoord.y);
vec2 noise=vec2(texture(noiseTile,(mod(floor(pixel),32.0)+0.5)/32.0).r,
texture(noiseTile,(mod(floor(pixel+vec2(37.0)),32.0)+0.5)/32.0).r);
float minLod=max(0.0,log2(min(size.x/rect.z,size.y/rect.w)));
color=blurPyramid(source,size,floor(log2(max(size.x,size.y)))+1.0,uv,
blurRadius*size.y/rect.w,minLod,noise,true)*opacity;
}`;

/** Premultiplied atlas sampling with the shared transparent-border mip kernel. */
export class KeycapGpu {
  private gl:WebGL2RenderingContext;
  private program:WebGLProgram;
  private noise:WebGLTexture;
  private textures=new Map<HTMLCanvasElement,WebGLTexture>();
  private locations=new Map<string,WebGLUniformLocation|null>();
  constructor(private canvas:HTMLCanvasElement) {
    const gl=canvas.getContext('webgl2',{alpha:true,premultipliedAlpha:true,antialias:false});
    if(!gl)throw new Error('GPU shortcut rendering is unavailable');
    this.gl=gl;
    const compile=(type:number,code:string)=>{
      const shader=gl.createShader(type)!;gl.shaderSource(shader,code);gl.compileShader(shader);
      if(!gl.getShaderParameter(shader,gl.COMPILE_STATUS)) {
        const error=gl.getShaderInfoLog(shader);gl.deleteShader(shader);throw new Error(error??'Shortcut shader failed');
      }
      return shader;
    };
    const vs=compile(gl.VERTEX_SHADER,vertex),fs=compile(gl.FRAGMENT_SHADER,fragment);
    const program=gl.createProgram()!;gl.attachShader(program,vs);gl.attachShader(program,fs);gl.linkProgram(program);
    gl.deleteShader(vs);gl.deleteShader(fs);
    if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw new Error(gl.getProgramInfoLog(program)??'Shortcut program failed');
    this.program=program;this.noise=gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D,this.noise);
    gl.texImage2D(gl.TEXTURE_2D,0,gl.R32F,32,32,0,gl.RED,gl.FLOAT,loupeNoise);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
  }
  private location(name:string) {
    if(!this.locations.has(name))this.locations.set(name,this.gl.getUniformLocation(this.program,name));
    return this.locations.get(name)!;
  }
  clear() {const gl=this.gl;gl.viewport(0,0,this.canvas.width,this.canvas.height);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);}
  resetAtlas() {for(const texture of this.textures.values())this.gl.deleteTexture(texture);this.textures.clear();}
  draw(atlas:KeycapAtlas,caps:readonly AnimatedKeycap[],blur=true) {
    const gl=this.gl;if(gl.isContextLost())return;
    gl.viewport(0,0,this.canvas.width,this.canvas.height);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program);gl.enable(gl.BLEND);gl.blendFunc(gl.ONE,gl.ONE_MINUS_SRC_ALPHA);
    gl.uniform2f(this.location('resolution'),this.canvas.width,this.canvas.height);
    gl.activeTexture(gl.TEXTURE1);gl.bindTexture(gl.TEXTURE_2D,this.noise);gl.uniform1i(this.location('noiseTile'),1);
    gl.activeTexture(gl.TEXTURE0);gl.uniform1i(this.location('source'),0);
    for(const cap of caps) {
      if(cap.opacity<=0)continue;
      const raster=atlas.get(cap.label),draw=keycapDraw(cap,raster.entry);
      let texture=this.textures.get(raster.canvas);
      if(!texture) {
        texture=gl.createTexture()!;this.textures.set(raster.canvas,texture);gl.bindTexture(gl.TEXTURE_2D,texture);
        gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL,false);gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL,false);
        const pixels=raster.canvas.getContext('2d')!.getImageData(0,0,raster.canvas.width,raster.canvas.height).data;
        const levels=keycapMipmaps(raster.canvas.width,raster.canvas.height,pixels);
        levels.forEach((level,index)=>gl.texImage2D(gl.TEXTURE_2D,index,gl.RGBA8,level.width,level.height,0,
          gl.RGBA,gl.UNSIGNED_BYTE,bottomUpPixels(level)));
        gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR_MIPMAP_LINEAR);
        gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
      } else gl.bindTexture(gl.TEXTURE_2D,texture);
      gl.uniform2f(this.location('size'),raster.canvas.width,raster.canvas.height);
      gl.uniform4f(this.location('rect'),draw.x,draw.y,draw.width,draw.height);
      gl.uniform1f(this.location('opacity'),draw.opacity);
      gl.uniform1f(this.location('blurRadius'),blur?5*(1-draw.opacity):0);
      gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
    }
  }
  destroy(){this.resetAtlas();this.gl.deleteTexture(this.noise);this.gl.deleteProgram(this.program);}
}
