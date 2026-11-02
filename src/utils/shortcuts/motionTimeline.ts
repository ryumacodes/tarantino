import type {EditorState} from '../../stores/editor/types';
import {sampleWebcam} from '../camera/webcamTimeline';
import {previewSample,timelineDuration} from '../timeline/previewTimeline';
import {KeycapMotion,type KeycapSourceTime} from './keycapMotion';
import {keycapInstances} from './keycapInstances';

export function shortcutMotionSource(state:EditorState,outputTime:number):KeycapSourceTime|null {
  const time=state.trimStart+outputTime;
  if(!Number.isFinite(time)||outputTime<0||time>=Math.min(state.trimEnd,timelineDuration(state)))return null;
  const sample=previewSample(state,time);
  if(sample.sourceTime===null||(sample.sourceFilePath&&sample.sourceFilePath!==state.videoFilePath))return null;
  const camera=state.hasWebcam?sampleWebcam(state.visualSettings,state.webcamKeyframes,sample.sourceTime):null;
  return {time:sample.sourceTime,fullscreenCamera:camera?.visible&&camera.layout==='fullscreen'};
}

export function createShortcutMotion(state:EditorState,width:number,height:number,fps:number) {
  const instances=keycapInstances(state.shortcutIntervals,new Set(state.shortcutSettings.hidden));
  return new KeycapMotion(instances,state.shortcutSettings,width,height,fps,time=>shortcutMotionSource(state,time));
}
