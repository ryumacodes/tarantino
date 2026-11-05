import { isMacRuntime } from '../../../utils/platform';
import { LatestTaskQueue } from '../../../utils/latestTaskQueue';
import {RecordedCursor} from './RecordedCursor';
import type {RecordedCursorData} from '../../../utils/motion/recordedCursor';
import {CursorTile} from './CursorTile';
import {loupeNoise} from './loupeNoise';
import {cursorTrailSamples} from '../../../utils/motion/cursorTrail';
import {previewCameraPose} from '../../../utils/motion/previewCamera';
import { exportTimeline } from '../../../utils/timeline/exportTimeline';
import {cursorShutter,cursorOutputScale,type PreviewCameraMotion} from '../../../utils/motion/cursorShutter';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import React, { useMemo, useRef, useEffect, useState } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { Effect, BlendFunction } from 'postprocessing';
import * as THREE from 'three';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { CAMERA_SIMULATION_FPS, cameraFrameIndex, sampleCameraFrame } from '../../../utils/motion/zoomTrajectory';
import { resolveCursorSpring } from '../../../utils/motion/cursorStyle';
import { cursorFragmentShader } from './shaders/cursor.glsl';

interface TrailPosition {
  x: number;
  y: number;
}

interface TrajectoryClock {
  outputTime: boolean;
  originMs: number;
  segments?: number[][];
  entryTimes?: number[];
}

function parseColor(hex: string): { r: number; g: number; b: number } {
  const match = hex.match(/^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i);
  if (match) {
    return {
      r: parseInt(match[1], 16) / 255,
      g: parseInt(match[2], 16) / 255,
      b: parseInt(match[3], 16) / 255,
    };
  }
  return { r: 0.39, g: 0.71, b: 1.0 };
}

export class CursorEffectImpl extends Effect {
  private tile:CursorTile|null=null;
  private recorded:RecordedCursor|null=null;
  setRecorded(data:RecordedCursorData|null) {
    this.recorded?.dispose();this.recorded=data?new RecordedCursor(data):null;
    this.uniforms.get('uCursorTile')!.value=null;
    this.uniforms.get('uCursorImageRect')!.value.set(0,0,0,0);
  }
  selectRecorded(time:number|null,enabled:boolean) {this.recorded?.select(time,enabled);}

  private noise:THREE.DataTexture|null=null;
  prepare(renderer:THREE.WebGLRenderer) {
    this.uniforms.get('uCursorImageRect')!.value.set(0,0,0,0);
    if(this.uniforms.get('uCursorOpacity')!.value<=0.01)return;
    if(!this.noise) {
      this.noise=new THREE.DataTexture(loupeNoise,32,32,THREE.RedFormat,THREE.FloatType);
      this.noise.needsUpdate=true;
      this.uniforms.get('uCursorNoise')!.value=this.noise;
    }
    const u=this.uniforms;
    const recorded=this.recorded?.prepare(renderer);
    if(recorded) {
      u.get('uCursorTile')!.value=recorded.texture;
      u.get('uCursorImageRect')!.value.fromArray(recorded.rect);
      u.get('uCursorImageSize')!.value.fromArray(recorded.size);
      return;
    }
    if(u.get('uCursorFadeBlur')!.value<=0.04)return;
    this.tile??=new CursorTile();
    this.tile.update(renderer,u.get('uCursorStyle')!.value,u.get('uShadowIntensity')!.value,
      ['R','G','B'].map(channel=>u.get((u.get('uIsClicking')!.value>.5?'uHighlightColor':'uCursorColor')+channel)!.value));
    u.get('uCursorTile')!.value=this.tile.pyramid.target.texture;
  }
  override update(renderer:THREE.WebGLRenderer) {this.prepare(renderer);}
  override dispose() {this.recorded?.dispose();this.tile?.dispose();this.noise?.dispose();super.dispose();}

  constructor() {
    const trailDefault = new Array(30).fill(null).map(() => new THREE.Vector4(0, 0, 0, 0));

    super('CursorEffect', cursorFragmentShader, {
      blendFunction: BlendFunction.NORMAL,
      uniforms: new Map<string, THREE.Uniform>([
        ['uCursorTemporalEnabled',new THREE.Uniform(0)],
        ['uCursorShutterStart',new THREE.Uniform(new THREE.Vector4())],
        ['uCursorShutterNow',new THREE.Uniform(new THREE.Vector4())],
        ['uCursorClipBefore',new THREE.Uniform(new THREE.Vector4())],
        ['uCursorClipNow',new THREE.Uniform(new THREE.Vector4())],
        ['uCursorX', new THREE.Uniform(0.0)],
        ['uCursorY', new THREE.Uniform(0.0)],
        ['uCursorScale', new THREE.Uniform(3.0)],
        ['uCursorOpacity', new THREE.Uniform(0.0)],
        ['uCursorFadeBlur',new THREE.Uniform(0)],
        ['uCursorTile',new THREE.Uniform(null)],
        ['uCursorImageRect',new THREE.Uniform(new THREE.Vector4())],
        ['uCursorImageSize',new THREE.Uniform(new THREE.Vector3())],
        ['uCursorNoise',new THREE.Uniform(null)],
        ['uCursorRotation', new THREE.Uniform(0.0)],
        ['uCursorStyle', new THREE.Uniform(0.0)],
        ['uIsClicking', new THREE.Uniform(0.0)],
        ['uClickEffect', new THREE.Uniform(2.0)],
        ['uCursorColorR', new THREE.Uniform(1.0)],
        ['uCursorColorG', new THREE.Uniform(1.0)],
        ['uCursorColorB', new THREE.Uniform(1.0)],
        ['uHighlightColorR', new THREE.Uniform(1.0)],
        ['uHighlightColorG', new THREE.Uniform(0.42)],
        ['uHighlightColorB', new THREE.Uniform(0.42)],
        ['uRippleColorR', new THREE.Uniform(0.39)],
        ['uRippleColorG', new THREE.Uniform(0.71)],
        ['uRippleColorB', new THREE.Uniform(1.0)],
        ['uShadowIntensity', new THREE.Uniform(30.0)],
        ['uRippleProgress', new THREE.Uniform(0.0)],
        ['uRippleX', new THREE.Uniform(0.0)],
        ['uRippleY', new THREE.Uniform(0.0)],
        ['uCircleHlProgress', new THREE.Uniform(0.0)],
        ['uCircleHlX', new THREE.Uniform(0.0)],
        ['uCircleHlY', new THREE.Uniform(0.0)],
        ['uTrailEnabled', new THREE.Uniform(0.0)],
        ['uTrailCount', new THREE.Uniform(0.0)],
        ['uTrailOpacity', new THREE.Uniform(0.5)],
        ['uTrailPoints', new THREE.Uniform(trailDefault)],
        ['uResolutionX', new THREE.Uniform(1920.0)],
        ['uResolutionY', new THREE.Uniform(1080.0)],
        ['uVideoClipEnabled', new THREE.Uniform(0.0)],
        ['uVideoClipMinX', new THREE.Uniform(0.0)],
        ['uVideoClipMinY', new THREE.Uniform(0.0)],
        ['uVideoClipMaxX', new THREE.Uniform(99999.0)],
        ['uVideoClipMaxY', new THREE.Uniform(99999.0)],
      ]),
    });
  }
}

interface CursorEffectProps {
  cameraMotionRef:React.MutableRefObject<PreviewCameraMotion|null>;
  effectRef?: React.MutableRefObject<CursorEffectImpl | null>;
  sidecarPath: string;
  videoWidth: number;
  videoHeight: number;
  visible: boolean;
  videoTransformRef: React.MutableRefObject<{
    scale: number;
    offsetX: number;
    offsetY: number;
    viewportWidth: number;
    viewportHeight: number;
    planeWidth: number;
    planeHeight: number;
  }>;
}

export const CursorEffect: React.FC<CursorEffectProps> = ({
  effectRef,
  cameraMotionRef,
  sidecarPath,
  videoWidth,
  videoHeight,
  visible,
  videoTransformRef,
}) => {
  const effect = useMemo(() => new CursorEffectImpl(), []);
  const alwaysUsePointer = useEditorStore(state => state.visualSettings.alwaysUsePointer);
  const cursorStyle = useEditorStore(state => state.visualSettings.cursorStyle);
  const followHover = useEditorStore(state => state.visualSettings.followHover ?? true);
  const optimizeCursorTypes = useEditorStore(state=>state.visualSettings.optimizeCursorTypes ?? true);
  useEffect(() => {
    let cancelled=false;
    effect.setRecorded(null);
    if(sidecarPath)invoke<RecordedCursorData>('load_cursor_images',{mouseJsonPath:sidecarPath,optimizeCursorTypes,alwaysUsePointer,style:cursorStyle})
      .then(data=>{if(!cancelled)effect.setRecorded(data);})
      .catch(error=>{if(!cancelled)console.warn('Could not load recorded cursor images',error);});
    return ()=>{cancelled=true;effect.setRecorded(null);};
  },[effect,sidecarPath,optimizeCursorTypes,alwaysUsePointer,cursorStyle]);
  useEffect(() => {
    if (effectRef) effectRef.current = effect;
    return () => { if (effectRef?.current === effect) effectRef.current = null; };
  }, [effect, effectRef]);
  const { gl } = useThree();
  const drawingSize=useMemo(()=>new THREE.Vector2(),[]);
  const timelineClips=useEditorStore(state=>state.clips);
  const timelineTracks=useEditorStore(state=>state.tracks);
  const trimStart=useEditorStore(state=>state.trimStart);
  const trimEnd=useEditorStore(state=>state.trimEnd);
  const recordingPath=useEditorStore(state=>state.videoFilePath);
  const trajectoryDuration=useEditorStore(state=>state.duration);
  const cursorRotation=useEditorStore(state=>state.visualSettings.cursorRotation);
  const rotateWhileMoving=useEditorStore(state=>state.visualSettings.rotateCursorWhileMoving);
  const rotationIntensity=useEditorStore(state=>state.visualSettings.rotationIntensity);
  const sourceCrop = useEditorStore(state => state.visualSettings.sourceCrop);
  const cursorScale = useEditorStore((state) => state.visualSettings.cursorScale);
  const hideCursorWhenIdle = useEditorStore((state) => state.visualSettings.hideCursorWhenIdle);
  const idleTimeout = useEditorStore((state) => state.visualSettings.idleTimeout);
  const stopCursorAtEnd = useEditorStore((state) => state.visualSettings.stopCursorAtEnd);
  const stopCursorDuration = useEditorStore((state) => state.visualSettings.stopCursorDuration);
  const cursorShakeThreshold = useEditorStore(state => state.visualSettings.removeCursorShakes ? state.visualSettings.shakesThreshold : 0);
  const loopCursorPosition = useEditorStore((state) => state.visualSettings.loopCursorPosition);

  const cursorMotionStyle = useEditorStore(state => state.visualSettings.cursorMotionStyle);
  const cursorSpeedPreset = useEditorStore(state => state.visualSettings.cursorSpeedPreset);
  const spring = resolveCursorSpring({ cursorMotionStyle, cursorSpeedPreset });

  const trajectoryContext = useMemo(() => ({
    sidecarPath, recordingPath, videoWidth, videoHeight, sourceCrop,
    timelineClips, timelineTracks, trimStart, trimEnd, trajectoryDuration,
  }), [sidecarPath, recordingPath, videoWidth, videoHeight, sourceCrop,
    timelineClips, timelineTracks, trimStart, trimEnd, trajectoryDuration]);
  const [trajectoryResult, setTrajectoryResult] = useState<{
    context: typeof trajectoryContext; frames: number[][]; clock: TrajectoryClock;
  } | null>(null);
  const trajectory = trajectoryResult?.context === trajectoryContext ? trajectoryResult.frames : null;
  const trajectoryQueue = useMemo(() => new LatestTaskQueue<string>(), []);
  const trailHistory = useRef<TrailPosition[]>([]);
  const priorTrajectory=useRef<number[][]|null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      if (!sidecarPath) return;

      try {
        const store = useEditorStore.getState();
        const duration = store.duration || 0;
        if (duration <= 0) return;

        const timeline=exportTimeline(store);
        const edit=timeline.timeline_clips && store.videoFilePath ? {
          recordingPath:store.videoFilePath,clips:timeline.timeline_clips,
          trimStart:timeline.trim_start,trimEnd:timeline.trim_end,
        }:null;
        const result = await trajectoryQueue.run(() => invoke<string>('compute_cursor_trajectory', {
          mouseJsonPath: sidecarPath,
          hover: recordingPath ? { recordingPath, enabled: isMacRuntime && followHover } : null,
          alwaysUsePointer,
          optimizeCursorTypes,
          durationMs: duration,
          fps: CAMERA_SIMULATION_FPS,
          videoWidth: videoWidth,
          videoHeight: videoHeight,
          cursorScale: cursorScale ?? 3.0,
          hideCursorWhenIdle,
          idleTimeout,
          stopCursorAtEnd,
          stopCursorDuration,
          loopCursorPosition,
          shakeThreshold: cursorShakeThreshold,
          sourceCrop: sourceCrop ?? null,
          springTension: spring.tension,
          springFriction: spring.friction,
          springMass: spring.mass,
          rotation:cursorRotation,rotateWhileMoving,rotationIntensity,edit,
        }));

        if (cancelled || result === undefined) return;

        const parsed=JSON.parse(result) as number[][]|{frames:number[][];outputTime:boolean;originMs:number;segments?:number[][]};
        const clock: TrajectoryClock = Array.isArray(parsed)?{outputTime:false,originMs:0}:parsed;
        const frames=Array.isArray(parsed)?parsed:parsed.frames;
        const spans=clock.segments??[[0,frames.length*1000/CAMERA_SIMULATION_FPS]];
        clock.entryTimes=spans.map(([start,end])=>{
          const last=Math.min(frames.length,Math.ceil(end*CAMERA_SIMULATION_FPS/1000));
          for(let i=Math.ceil(start*CAMERA_SIMULATION_FPS/1000);i<last;i++) {
            if(frames[i][2]>.01)return start+Math.floor((i*1000/CAMERA_SIMULATION_FPS-start)*CAMERA_SIMULATION_FPS/1000+1e-7)*1000/CAMERA_SIMULATION_FPS;
          }
          return Infinity;
        });
        setTrajectoryResult({context: trajectoryContext, frames, clock});
      } catch (error) {
        if (!cancelled) console.error('[CursorEffect] Failed to compute cursor trajectory:', error);
      }
    };

    load();
    return () => { cancelled = true; trajectoryQueue.clearPending(); };
  }, [
    sidecarPath,trajectoryDuration,timelineClips,timelineTracks,trimStart,trimEnd,recordingPath,
    videoWidth,
    videoHeight,
    cursorScale, alwaysUsePointer, optimizeCursorTypes, followHover,
    hideCursorWhenIdle,
    idleTimeout,
    stopCursorAtEnd,
    stopCursorDuration,
    loopCursorPosition,
    cursorShakeThreshold, sourceCrop,
    spring.tension, spring.friction, spring.mass,cursorRotation,rotateWhileMoving,rotationIntensity,
    trajectoryContext, trajectoryQueue,
  ]);

  useFrame(() => {
    effect.uniforms.get('uCursorTemporalEnabled')!.value=0;
    effect.selectRecorded(null,false);
    if (!visible || !trajectory || trajectory.length === 0) {
      trailHistory.current=[];priorTrajectory.current=null;
      for (const name of ['uCursorOpacity', 'uRippleProgress', 'uCircleHlProgress', 'uTrailCount']) {
        const uniform = effect.uniforms.get(name);
        if (uniform) uniform.value = 0;
      }
      return;
    }

    const store = useEditorStore.getState();
    const vs = store.visualSettings;
    // The editor synchronizes this value to the media clock every animation
    // frame. Using one clock keeps cursor sampling, timeline, zoom, and export
    // on identical millisecond timestamps.
    const sample = previewSample(store, store.currentTime);
    const clock=trajectoryResult!.clock;
    const currentTime=clock.outputTime?store.currentTime-clock.originMs:sample.sourceTime;
    if (currentTime===null || (clock.outputTime && (currentTime<0 || currentTime*CAMERA_SIMULATION_FPS/1000>=trajectory.length))
      || sample.sourceTime === null || sample.hideCursor || vs.hideCursor) {
      trailHistory.current=[];priorTrajectory.current=null;
      for (const name of ['uCursorOpacity', 'uRippleProgress', 'uCircleHlProgress', 'uTrailCount']) {
        const uniform = effect.uniforms.get(name);
        if (uniform) uniform.value = 0;
      }
      return;
    }

    const frameIdx = cameraFrameIndex(currentTime, trajectory.length);
    const frame = trajectory[Math.max(0, frameIdx)];
    effect.selectRecorded(sample.sourceTime,true);
    const cursorX = frame[0];
    const cursorY = frame[1];
    const opacity = vs.hideCursor ? 0 : frame[2];
    const isClicking = frame[3] > 0.5;
    const rippleProgress = frame[4];
    const rippleNormX = frame[5];
    const rippleNormY = frame[6];

    const effectiveCursorStyle = vs.cursorStyle;

    const transform = videoTransformRef.current;
    const toScreenUV = (normX: number, normY: number) => {
      const lx = normX - 0.5;
      const ly = 0.5 - normY;
      const wx = lx * transform.planeWidth * transform.scale + transform.offsetX;
      const wy = ly * transform.planeHeight * transform.scale + transform.offsetY;
      return {
        u: 0.5 + wx / transform.viewportWidth,
        v: 0.5 + wy / transform.viewportHeight,
      };
    };

    const { u: screenU, v: screenV } = toScreenUV(cursorX, cursorY);

    gl.getDrawingBufferSize(drawingSize);
    const outputSize=store.getExportDimensions();
    const densityScale=cursorOutputScale(drawingSize.x,drawingSize.y,outputSize.width,outputSize.height);
    const windowZoomScale = store.captureMode === 'window' ? transform.scale : 1;
    const adjustedCursorScale = vs.cursorScale * densityScale * windowZoomScale;

    if (vs.cursorTrailEnabled && opacity > 0.01) {
      const segmentIndex=clock.outputTime?clock.segments?.findIndex(([start,end])=>start<=currentTime && currentTime<end)??0:0;
      const segmentStart=clock.segments?.[segmentIndex]?.[0]??0;
      trailHistory.current=cursorTrailSamples(trajectory,currentTime,segmentStart,vs.cursorTrailLength,clock.entryTimes?.[segmentIndex]??Infinity).map(point=>{
        let pose=transform;
        if(store.captureMode!=='window') {
          const sourceTime=clock.outputTime?previewSample(store,point.time+clock.originMs).sourceTime:point.time;
          const camera=sampleCameraFrame(cameraMotionRef.current?.frames??[],sourceTime??0);
          pose={...transform,...previewCameraPose(camera[0],camera[1],camera[2],false,transform.planeWidth,transform.planeHeight)};
        }
        return {x:0.5+((point.frame[0]-.5)*pose.planeWidth*pose.scale+pose.offsetX)/pose.viewportWidth,
          y:0.5+((.5-point.frame[1])*pose.planeHeight*pose.scale+pose.offsetY)/pose.viewportHeight};
      });
    } else {
      trailHistory.current = [];
    }

    const { u: rippleScreenX, v: rippleScreenY } = rippleProgress > 0.001
      ? toScreenUV(rippleNormX, rippleNormY)
      : { u: 0, v: 0 };

    const styleMap: Record<string, number> = { pointer: 0, circle: 1, filled: 2, outline: 3, dotted: 4 };
    const styleVal = styleMap[effectiveCursorStyle] ?? 0;

    const clickEffectMap: Record<string, number> = { none: 0, circle: 1, ripple: 2 };
    const clickEffectVal = clickEffectMap[vs.clickEffect] ?? 2;

    const cursorCol = parseColor(isClicking ? vs.cursorHighlightColor : vs.cursorColor);
    const hlCol = parseColor(vs.cursorHighlightColor);
    const rippleCol = parseColor(vs.cursorRippleColor);

    const resX = drawingSize.x;
    const resY = drawingSize.y;

    const u = effect.uniforms;
    u.get('uCursorX')!.value = screenU;
    u.get('uCursorY')!.value = screenV;
    u.get('uCursorScale')!.value = adjustedCursorScale * (1 - (frame[9] ?? 0));
    u.get('uCursorOpacity')!.value = opacity;
    u.get('uCursorFadeBlur')!.value = (frame[8]??0)*densityScale;
    u.get('uCursorRotation')!.value = frame[7] ?? 0;
    u.get('uCursorStyle')!.value = styleVal;
    u.get('uIsClicking')!.value = isClicking ? 1.0 : 0.0;
    u.get('uClickEffect')!.value = clickEffectVal;
    u.get('uCursorColorR')!.value = cursorCol.r;
    u.get('uCursorColorG')!.value = cursorCol.g;
    u.get('uCursorColorB')!.value = cursorCol.b;
    u.get('uHighlightColorR')!.value = hlCol.r;
    u.get('uHighlightColorG')!.value = hlCol.g;
    u.get('uHighlightColorB')!.value = hlCol.b;
    u.get('uRippleColorR')!.value = rippleCol.r;
    u.get('uRippleColorG')!.value = rippleCol.g;
    u.get('uRippleColorB')!.value = rippleCol.b;
    u.get('uShadowIntensity')!.value = vs.cursorShadowIntensity;
    // Route animation data to the correct effect uniforms
    if (clickEffectVal === 1) {
      // Circle: use ripple animation data for circle highlight
      u.get('uRippleProgress')!.value = 0;
      u.get('uRippleX')!.value = 0;
      u.get('uRippleY')!.value = 0;
      u.get('uCircleHlProgress')!.value = rippleProgress;
      u.get('uCircleHlX')!.value = rippleScreenX;
      u.get('uCircleHlY')!.value = rippleScreenY;
    } else {
      // Ripple or None
      u.get('uRippleProgress')!.value = rippleProgress;
      u.get('uRippleX')!.value = rippleScreenX;
      u.get('uRippleY')!.value = rippleScreenY;
      u.get('uCircleHlProgress')!.value = 0;
      u.get('uCircleHlX')!.value = 0;
      u.get('uCircleHlY')!.value = 0;
    }
    u.get('uTrailEnabled')!.value = vs.cursorTrailEnabled ? 1.0 : 0.0;
    u.get('uTrailCount')!.value = trailHistory.current.length;
    u.get('uTrailOpacity')!.value = vs.cursorTrailOpacity;
    u.get('uResolutionX')!.value = resX;
    u.get('uResolutionY')!.value = resY;
    const camera=cameraMotionRef.current;
    if(camera && camera.timelineTime===store.currentTime && camera.revision===store.playbackRevision
      && camera.beforeSourceTime!==null && priorTrajectory.current===trajectory) {
      const priorTime=clock.outputTime?store.currentTime-clock.originMs-1000/camera.fps:camera.beforeSourceTime;
      const prior=trajectory[cameraFrameIndex(Math.max(0,priorTime),trajectory.length)];
      const shutter=cursorShutter(frame,prior,camera,{width:resX,height:resY,
        viewportWidth:transform.viewportWidth,viewportHeight:transform.viewportHeight,
        planeWidth:transform.planeWidth,planeHeight:transform.planeHeight,
        size:vs.cursorScale*densityScale,windowMode:store.captureMode==='window'},
        vs.motionBlurPanIntensity/100,vs.motionBlurZoomIntensity/100,vs.motionBlurEnabled && priorTime>=0);
      u.get('uCursorTemporalEnabled')!.value=Number(shutter.active);
      u.get('uCursorShutterStart')!.value.fromArray(shutter.before);
      u.get('uCursorShutterNow')!.value.fromArray(shutter.now);
      u.get('uCursorClipBefore')!.value.fromArray(shutter.clipBefore);
      u.get('uCursorClipNow')!.value.fromArray(shutter.clipNow);
    }
    effect.prepare(gl);
    priorTrajectory.current=opacity>.01?trajectory:null;


    u.get('uVideoClipEnabled')!.value = sourceCrop ? 1.0 : 0.0;
    if (sourceCrop) {
      // Window mode clips in its moving layer; display mode uses the fixed
      // source viewport, matching the compositor's inverse-sampling boundary.
      const a = store.captureMode === 'window' ? toScreenUV(0,1) : {
        u:0.5-transform.planeWidth/transform.viewportWidth/2,
        v:0.5-transform.planeHeight/transform.viewportHeight/2,
      };
      const b = store.captureMode === 'window' ? toScreenUV(1,0) : {
        u:0.5+transform.planeWidth/transform.viewportWidth/2,
        v:0.5+transform.planeHeight/transform.viewportHeight/2,
      };
      u.get('uVideoClipMinX')!.value = a.u*resX;
      u.get('uVideoClipMinY')!.value = a.v*resY;
      u.get('uVideoClipMaxX')!.value = b.u*resX;
      u.get('uVideoClipMaxY')!.value = b.v*resY;
    }

    // Trail points
    const trailUniform = u.get('uTrailPoints')!.value as THREE.Vector4[];
    for (let i = 0; i < 30; i++) {
      if (i < trailHistory.current.length) {
        const tp = trailHistory.current[i];
        const progress = i / trailHistory.current.length;
        trailUniform[i].set(tp.x, tp.y, progress * vs.cursorTrailOpacity * opacity, (2 + progress * 4) * adjustedCursorScale);
      } else {
        trailUniform[i].set(0, 0, 0, 0);
      }
    }
  });

  return <primitive object={effect} />;
};

CursorEffect.displayName = 'CursorEffect';
