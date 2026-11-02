import { VideoTexture, LinearFilter } from 'three';
import { useEditorStore } from '../../../stores/editor';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { sampleWebcam } from '../../../utils/camera/webcamTimeline';
import { containWebcam } from '../../../utils/camera/containWebcam';
import type { SilenceRange } from '../../../utils/camera/silence';
import type { CameraLayerFrame } from './CameraLayerPass';
import type { CameraOpacityFrame } from './hooks/useCameraTrajectory';

/** One decoded video and texture shared by the stage and lens foreground. */
export class CameraPreviewSource {
  private video:HTMLVideoElement|null=null;
  private texture:VideoTexture|null=null;
  ready=false;
  opacity:readonly CameraOpacityFrame[]=[];
  silence:readonly SilenceRange[]=[];
  setVideo=(video:HTMLVideoElement|null)=>{
    if(video===this.video)return;
    this.texture?.dispose();this.texture=null;this.video=video;
    if(video){this.texture=new VideoTexture(video);this.texture.minFilter=this.texture.magFilter=LinearFilter;this.texture.needsUpdate=true;}
  };
  frame(width:number,height:number,lens=false):CameraLayerFrame|null {
    if(!this.ready || !this.video || !this.texture || this.video.readyState<2)return null;
    const state=useEditorStore.getState(),sample=previewSample(state,state.currentTime);
    if(sample.sourceTime===null || !state.hasWebcam)return null;
    const camera=sampleWebcam(state.visualSettings,state.webcamKeyframes,sample.sourceTime,this.silence);
    const opacity=this.opacity[Math.floor(sample.sourceTime*120/1000+1e-7)];
    if(!camera.visible && !opacity?.rect)return null;
    const bounds=containWebcam(camera.x,camera.y,camera.size,width,height);
    const side=bounds.size*width;
    return {texture:this.texture,sourceWidth:this.video.videoWidth,sourceHeight:this.video.videoHeight,
      rect:opacity?.rect?[opacity.rect[0]*width,opacity.rect[1]*height,opacity.rect[2]*width,opacity.rect[3]*height]:camera.layout==='fullscreen'?[0,0,width,height]:[bounds.x*width-side/2,bounds.y*height-side/2,side,side],
      shape:camera.layout==='fullscreen'?'fullscreen':camera.shape,
      mirrored:state.visualSettings.webcamMirrored ?? false,
      opacity:opacity ? opacity.stage*(lens?opacity.lens_content:1):1};
  }
  dispose(){this.setVideo(null);}
}
