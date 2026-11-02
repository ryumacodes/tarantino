import {
  Camera, Color, Object3D, Scene, Vector4, WebGLRenderer, WebGLRenderTarget,
} from 'three';

/** Mark backdrop drawables without taking over the application's camera layers. */
export const LOUPE_BACKDROP = 'loupeBackdrop';

/** Cached, premultiplied scene inputs. Post-processing (including cursor) follows capture. */
export class LoupeSceneLayers {
  readonly foreground: WebGLRenderTarget;
  readonly background: WebGLRenderTarget;

  constructor(width: number, height: number) {
    this.foreground = new WebGLRenderTarget(width, height);
    this.background = new WebGLRenderTarget(width, height);
    this.foreground.texture.name = 'Magnifier foreground';
    this.background.texture.name = 'Magnifier backdrop';
  }

  setSize(width: number, height: number) {
    this.foreground.setSize(width, height);
    this.background.setSize(width, height);
  }

  capture(renderer: WebGLRenderer, scene: Scene, camera: Camera) {
    const target = renderer.getRenderTarget();
    const face = renderer.getActiveCubeFace();
    const mip = renderer.getActiveMipmapLevel();
    const viewport = renderer.getViewport(new Vector4());
    const scissor = renderer.getScissor(new Vector4());
    const scissorTest = renderer.getScissorTest();
    const clearColor = renderer.getClearColor(new Color());
    const clearAlpha = renderer.getClearAlpha();
    const autoClear = renderer.autoClear;
    const sceneBackground = scene.background;
    const masks: [Object3D, number, boolean][] = [];
    // A zero layer mask skips a drawable without hiding its children.
    // Preserve existing camera filters and hidden parent groups.
    scene.traverse(object => {
      if ('isMesh' in object || 'isLine' in object || 'isPoints' in object || 'isSprite' in object) {
        let backdrop = false;
        for (let parent: Object3D | null = object; parent; parent = parent.parent) {
          if (parent.userData[LOUPE_BACKDROP]) { backdrop = true; break; }
        }
        masks.push([object, object.layers.mask, backdrop]);
      }
    });
    try {
      renderer.autoClear = false;
      renderer.setScissorTest(false);
      renderer.setClearColor(0, 0);
      for (const backdrop of [false, true]) {
        const destination = backdrop ? this.background : this.foreground;
        scene.background = backdrop ? sceneBackground : null;
        for (const [object, mask, isBackdrop] of masks) {
          object.layers.mask = isBackdrop === backdrop ? mask : 0;
        }
        renderer.setRenderTarget(destination);
        // setRenderTarget applies its pixel-sized viewport. setViewport would
        // multiply these dimensions by the display pixel ratio a second time.
        renderer.clear(true, true, true);
        renderer.render(scene, camera);
      }
    } finally {
      for (const [object, mask] of masks) object.layers.mask = mask;
      scene.background = sceneBackground;
      renderer.setClearColor(clearColor, clearAlpha);
      renderer.autoClear = autoClear;
      renderer.setRenderTarget(target, face, mip);
      renderer.setViewport(viewport);
      renderer.setScissor(scissor);
      renderer.setScissorTest(scissorTest);
    }
  }

  dispose() {
    this.foreground.dispose();
    this.background.dispose();
  }
}
