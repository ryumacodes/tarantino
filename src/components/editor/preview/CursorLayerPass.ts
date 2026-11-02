import { BlendFunction, Effect, EffectPass } from 'postprocessing';
import { Camera, UnsignedByteType, Vector4, WebGLRenderTarget, type Uniform, type WebGLRenderer } from 'three';
import { cursorFragmentShader } from './shaders/cursor.glsl';

/** Reuses the visible cursor's current uniforms without recomputing its trajectory. */
export class CursorLayerPass {
  readonly target: WebGLRenderTarget;
  private readonly pass: EffectPass;
  private initialized = false;

  constructor(uniforms: Map<string, Uniform>, width: number, height: number) {
    const effect = new Effect('ForegroundCursor', cursorFragmentShader, {
      blendFunction: BlendFunction.SRC,
      defines: new Map([['CURSOR_PREMULTIPLIED_INPUT', '1']]),
      uniforms: new Map(uniforms),
    });
    this.pass = new EffectPass(new Camera(), effect);
    this.target = new WebGLRenderTarget(width, height, { depthBuffer: false });
    this.target.texture.name = 'Magnifier foreground with cursor';
    this.pass.setSize(width, height);
  }

  render(renderer: WebGLRenderer, input: WebGLRenderTarget) {
    if (!this.initialized) {
      this.pass.initialize(renderer, false, UnsignedByteType);
      this.initialized = true;
    }
    if (this.target.width !== input.width || this.target.height !== input.height) {
      this.target.setSize(input.width, input.height);
      this.pass.setSize(input.width, input.height);
    }
    const target = renderer.getRenderTarget();
    const face = renderer.getActiveCubeFace(), mip = renderer.getActiveMipmapLevel();
    const viewport = renderer.getViewport(new Vector4());
    const scissor = renderer.getScissor(new Vector4()), scissorTest = renderer.getScissorTest();
    try {
      renderer.setScissorTest(false);
      this.pass.render(renderer, input, this.target, 0, false);
    } finally {
      renderer.setRenderTarget(target, face, mip);
      renderer.setViewport(viewport);
      renderer.setScissor(scissor);
      renderer.setScissorTest(scissorTest);
    }
    return this.target.texture;
  }

  dispose() {
    this.pass.dispose();
    this.target.dispose();
  }
}
