import {previewSample,type TimelineState} from '../timeline/previewTimeline';
import {sampleCameraFrame,type CameraFrame} from './zoomTrajectory';

/** Sample the same two source poses used by adjacent exported output frames. */
export function previewCameraPair(state:TimelineState,timeMs:number,
  frames:readonly CameraFrame[],frameRate:number) {
  const fps=Number.isFinite(frameRate)&&frameRate>0?frameRate:60;
  const sample=previewSample(state,timeMs);
  const priorTime=timeMs-1000/fps;
  const prior=priorTime>=0?previewSample(state,priorTime):null;
  const canBlur=sample.sourceTime!==null && prior?.sourceTime!=null
    && prior.clipId===sample.clipId && prior.sourceFilePath===sample.sourceFilePath
    && prior.rate===sample.rate;
  const now:CameraFrame=sample.sourceTime===null?[1,.5,.5]:sampleCameraFrame(frames,sample.sourceTime);
  const before=canBlur?sampleCameraFrame(frames,prior.sourceTime!):now;
  return {sample,now,before,canBlur,fps,beforeSourceTime:canBlur?prior!.sourceTime:sample.sourceTime,
    beforeCursorHidden:prior?.hideCursor??true};
}
