import type { Camera, Scene, Uniform, WebGLRenderer } from 'three';
import { CameraLayerPass } from './CameraLayerPass';
import type { CameraPreviewSource } from './CameraPreviewSource';
import { CursorLayerPass } from './CursorLayerPass';
import { LoupeSceneLayers } from './LoupeSceneLayers';

/** Owns offscreen resources, borrowing only scene, camera and live cursor uniforms. */
export class LoupePreviewLayers {
  private readonly sceneLayers: LoupeSceneLayers;
  private cameraPass: CameraLayerPass | null = null;
  private cursorPass: CursorLayerPass | null = null;
  private cursorUniforms: Map<string, Uniform> | null = null;

  constructor(width: number, height: number) {
    this.sceneLayers = new LoupeSceneLayers(width, height);
  }

  capture(renderer: WebGLRenderer, scene: Scene, camera: Camera,
    width: number, height: number, cursorUniforms: Map<string, Uniform> | null, cameraSource?: CameraPreviewSource) {
    this.sceneLayers.setSize(width, height);
    this.sceneLayers.capture(renderer, scene, camera);
    if (cursorUniforms !== this.cursorUniforms) {
      this.cursorPass?.dispose();
      this.cursorUniforms = cursorUniforms;
      this.cursorPass = cursorUniforms ? new CursorLayerPass(cursorUniforms, width, height) : null;
    }
    let foreground=this.sceneLayers.foreground;
    if(this.cursorPass){this.cursorPass.render(renderer,foreground);foreground=this.cursorPass.target;}
    const frame=cameraSource?.frame(width,height,true);
    if(frame){
      this.cameraPass ??= new CameraLayerPass(width,height);
      this.cameraPass.render(renderer,foreground,frame);
      foreground=this.cameraPass.target;
    }
    return {foreground:foreground.texture,background:this.sceneLayers.background.texture};
  }

  dispose() {
    this.cursorPass?.dispose();
    this.cameraPass?.dispose();
    this.sceneLayers.dispose();
  }
}
