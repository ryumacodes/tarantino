import {MaskBoundsEditor} from './MaskBoundsEditor';
import type {PreviewCameraMotion} from '../../../utils/motion/cursorShutter';
import {continuousPreviewMotion,type PreviewMotionSample} from '../../../utils/motion/previewContinuity';
import {windowShutter} from '../../../utils/motion/windowShutter';
import {TemporalWindowPreview,type TemporalWindowFrame} from './TemporalWindowPreview';
import { previewCameraFramePose, previewCameraVelocity } from '../../../utils/motion/previewCamera';
import type { SourceCrop } from '../../../utils/sourceCrop';
import { resolveSourceCrop } from '../../../utils/sourceCrop';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { useShallow } from 'zustand/react/shallow';
import { selectVideoInputs } from './hooks/videoInputs';
import React, { useRef, useState, useEffect, useMemo, Suspense } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import * as THREE from 'three';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../stores/editor';
import { VideoMaterial, VideoFallback, LinuxNativeVideoOverlay } from './VideoMaterial';
import { BackgroundPlane } from './BackgroundPlane';
import { VideoShadow } from './VideoShadow';
import { isLinuxRuntime } from '../../../utils/platform';

import { contentInset, previewGeometry } from '../../../utils/previewGeometry';
import { useCameraTrajectory } from './hooks/useCameraTrajectory';
import {previewCameraPair} from '../../../utils/motion/previewCameraPair';

interface VideoTransform {
  scale: number;
  scaleY?: number;
  offsetX: number;
  offsetY: number;
  viewportWidth: number;
  viewportHeight: number;
  planeWidth: number;
  planeHeight: number;
}

interface VideoViewerProps {
  cameraMotionRef:React.MutableRefObject<PreviewCameraMotion|null>;
  temporalWindowActive: React.MutableRefObject<boolean>;
  showMouseOverlay: boolean;
  isPlaying: boolean;
  velocityRef: React.MutableRefObject<{ scale: number; x: number; y: number }>;
  videoTransformRef: React.MutableRefObject<VideoTransform>;
}

export const VideoViewer = React.memo(function VideoViewer({
  showMouseOverlay,
  cameraMotionRef,
  temporalWindowActive,
  isPlaying,
  velocityRef,
  videoTransformRef
}: VideoViewerProps) {
  const {
    videoFilePath,
    loadMouseEvents,
    visualSettings,
    displayResolution,
    screenResolution,
    recordingArea,
    videoWidth,
    videoHeight,
    captureMode,
    outputWidth, outputHeight,
  } = useEditorStore(useShallow(selectVideoInputs));
  const displayViewRef=useRef<SourceCrop>({x:0,y:0,width:1,height:1});
  const meshRef = useRef<THREE.Mesh>(null);
  const groupRef = useRef<THREE.Group>(null);
  const { viewport,gl } = useThree();
  const nativeMaskRef=useRef<THREE.Texture|null>(null);
  const shadowTextureRef=useRef<THREE.Texture|null>(null);
  const shutterRef=useRef<ReturnType<typeof windowShutter>|null>(null);
  const renderSize=useRef({width:1,height:1,pixelsPerUnit:1});
  const whiteMask=useMemo(()=>{
    const texture=new THREE.DataTexture(new Uint8Array([255,255,255,255]),1,1);
    texture.needsUpdate=true;return texture;
  },[]);
  useEffect(()=>()=>whiteMask.dispose(),[whiteMask]);
  const bufferSize=useMemo(()=>new THREE.Vector2(),[]);

  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoError, setVideoError] = useState<string | null>(null);
  const [linuxNativeOverlayEnabled, setLinuxNativeOverlayEnabled] = useState(false);

  useEffect(() => {
    const loadVideo = async () => {
      if (videoFilePath) {
        try {
          const url = convertFileSrc(videoFilePath);
          setVideoUrl(url);
          setVideoError(null);

          const mouseFilePath = videoFilePath.replace('.mp4', '.mouse.json');
          loadMouseEvents(mouseFilePath);
        } catch (err) {
          console.error('VideoViewer: Failed to convert file path:', err);
          setVideoError(`Failed to load video: ${err}`);
        }
      }
    };
    loadVideo();
  }, [videoFilePath, loadMouseEvents]);

  const output = { width: outputWidth, height: outputHeight };
  const originalSource = { width: videoWidth || displayResolution?.width || 1920,
    height: videoHeight || displayResolution?.height || 1080 };
  const crop = visualSettings.sourceCrop
    ? resolveSourceCrop(visualSettings.sourceCrop,originalSource.width,originalSource.height) : undefined;
  const geometry = previewGeometry({
    output,
    source: crop ?? originalSource,
    viewport: { width: viewport.width, height: viewport.height },
    padding: visualSettings.padding,
    desktop: captureMode === 'window' && visualSettings.windowLayoutMode === 'desktop'
      && screenResolution && recordingArea
      ? { screen: screenResolution, window: crop ? {
          width: Math.max(1,Math.round(recordingArea.width*crop.width/originalSource.width)),
          height: Math.max(1,Math.round(recordingArea.height*crop.height/originalSource.height)),
        } : recordingArea } : undefined,
  });
  const { canvasWidth: basePlaneWidth, canvasHeight: basePlaneHeight, planeWidth, planeHeight } = geometry;

  const paddedWidth=output.width-2*contentInset(output.width,output.height,visualSettings.padding);
  const paddedHeight=output.height-2*contentInset(output.width,output.height,visualSettings.padding);
  const outputCornerRadius=visualSettings.cornerRadius>0
    ?Math.max(1,Math.min(paddedWidth,paddedHeight)*visualSettings.cornerRadius/200):0;
  const inputShortSide=Math.min(planeWidth,planeHeight)*output.width/basePlaneWidth;

  const { frames: cameraFrames, error: motionError } = useCameraTrajectory();

  const previousMotion = useRef<PreviewMotionSample|null>(null);
  useEffect(()=>()=>{cameraMotionRef.current=null;},[cameraMotionRef]);
  useFrame(() => {
    const state = useEditorStore.getState();
    window.__TARANTINO_CURRENT_TIME = previewSample(state, state.currentTime).sourceTime ?? 0;
    if (!meshRef.current) {cameraMotionRef.current=null;return;}
    const frameRate=state.exportSettings.format==='gif'?Math.min(state.exportSettings.frameRate,30):state.exportSettings.frameRate;
    const pair=previewCameraPair(state,state.currentTime,cameraFrames,frameRate);
    const {sample}=pair;
    const motionSample:PreviewMotionSample={playing:state.isPlaying,revision:state.playbackRevision,
      clipId:sample.clipId,sourceTime:sample.sourceTime,sourcePath:sample.sourceFilePath??state.videoFilePath,
      rate:sample.rate,trajectory:cameraFrames,clips:state.clips,tracks:state.tracks};
    const continuous=pair.canBlur&&continuousPreviewMotion(previousMotion.current,motionSample);
    previousMotion.current=motionSample;
    const previous = pair.before;
    const pose = previewCameraFramePose(pair.now,
      captureMode==='window',planeWidth,planeHeight);
    const before = previewCameraFramePose(previous,
      captureMode==='window',planeWidth,planeHeight);
    cameraMotionRef.current=motionError||!videoFilePath?null:{now:pose,before,frames:cameraFrames,
      beforeSourceTime:pair.beforeSourceTime,beforeCursorHidden:pair.beforeCursorHidden,
      fps:pair.fps,continuous:isPlaying&&continuous,timelineTime:state.currentTime,revision:state.playbackRevision};
    gl.getDrawingBufferSize(bufferSize);
    const renderWidth=Math.max(1,Math.round(bufferSize.x*basePlaneWidth/viewport.width));
    const renderHeight=Math.max(1,Math.round(bufferSize.y*basePlaneHeight/viewport.height));
    const pixelsPerUnit=renderWidth/basePlaneWidth;
    renderSize.current={width:renderWidth,height:renderHeight,pixelsPerUnit};
    shutterRef.current=windowShutter(pose,before,{width:renderWidth,height:renderHeight,
      planeWidth,planeHeight,pixelsPerUnit},{continuous:isPlaying&&continuous,
      samplesPerSecond:pair.fps,rate:1,
      pan:visualSettings.motionBlurPanIntensity/100,zoom:visualSettings.motionBlurZoomIntensity/100});
    const {offsetX,offsetY}=pose;
    displayViewRef.current=pose.view;
    velocityRef.current=previewCameraVelocity(pose,before,planeWidth,planeHeight,
      pair.fps,1,isPlaying && continuous);

    if (captureMode === 'window') {
      if (groupRef.current) {
        groupRef.current.scale.set(pose.scale, pose.scaleY??pose.scale, 1);
        groupRef.current.position.set(offsetX, offsetY, 0);
      }
      meshRef.current.scale.set(planeWidth, planeHeight, 1);
      meshRef.current.position.set(0, 0, 0);
    } else {
      if (groupRef.current) {
        groupRef.current.scale.set(1, 1, 1);
        groupRef.current.position.set(0, 0, 0);
      }
      meshRef.current.scale.set(planeWidth,planeHeight,1);
      meshRef.current.position.set(0,0,0);
    }

    videoTransformRef.current.scale = pose.scale;
    videoTransformRef.current.scaleY = pose.scaleY??pose.scale;
    videoTransformRef.current.offsetX = offsetX;
    videoTransformRef.current.offsetY = offsetY;
    videoTransformRef.current.viewportWidth = viewport.width;
    videoTransformRef.current.viewportHeight = viewport.height;
    videoTransformRef.current.planeWidth = planeWidth;
    videoTransformRef.current.planeHeight = planeHeight;
  },-2);

  const temporalFrame=():TemporalWindowFrame|null=>{
    const shutter=shutterRef.current,material=meshRef.current?.material;
    if(captureMode!=='window'||!visualSettings.motionBlurEnabled||!shutter?.active||linuxNativeOverlayEnabled
      ||!(material instanceof THREE.MeshBasicMaterial)||!material.map
      ||(visualSettings.shadowEnabled&&!shadowTextureRef.current))return null;
    const {width,height,pixelsPerUnit}=renderSize.current;
    const inputWidth=planeWidth*pixelsPerUnit,inputHeight=planeHeight*pixelsPerUnit;
    const contentWidth=output.width-2*contentInset(output.width,output.height,visualSettings.padding);
    const contentHeight=output.height-2*contentInset(output.width,output.height,visualSettings.padding);
    const radius=visualSettings.cornerRadius>0?Math.max(1,Math.min(contentWidth,contentHeight)*visualSettings.cornerRadius/200):0;
    return {width,height,input:{source:material.map,mask:nativeMaskRef.current??whiteMask,
      shadow:shadowTextureRef.current??whiteMask,before:shutter.before,now:shutter.now,
      inputSize:[inputWidth,inputHeight],baseCenter:[width/2,height/2],
      sourceSize:[(material.map.isRenderTargetTexture?(material.map.image as {width:number}).width:originalSource.width)*material.map.repeat.x,
        (material.map.isRenderTargetTexture?(material.map.image as {height:number}).height:originalSource.height)*material.map.repeat.y],
      cornerRadius:radius*width/output.width,hasNativeMask:!!nativeMaskRef.current,
      shadowEnabled:visualSettings.shadowEnabled,sourceTransform:material.map.matrix,
      maskTransform:nativeMaskRef.current?.matrix}};
  };


  if (!videoFilePath) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
      </group>
    );
  }

  if (videoError || motionError) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
        <Html center>
          <div role="alert" style={{ color: '#ff5555', width: 280, textAlign: 'center' }}>
            {videoError ?? motionError}
          </div>
        </Html>
      </group>
    );
  }

  if (!videoUrl) {
    return (
      <group ref={groupRef}>
        <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
        <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
          <planeGeometry args={[1, 1]} />
          <meshBasicMaterial color="#1a1a1a" toneMapped={false} />
        </mesh>
        {/* Keep loading UI independent of font loading: suspending this
            component prevents its effect from starting the media load. */}
        <Html center>
          <div role="status" style={{ color: '#6272a4', whiteSpace: 'nowrap' }}>
            Loading video...
          </div>
        </Html>
      </group>
    );
  }

  return (
    <>
      <BackgroundPlane width={basePlaneWidth} height={basePlaneHeight} settings={visualSettings} />
      <group ref={groupRef}>
      <VideoShadow textureRef={shadowTextureRef} canvasWidth={basePlaneWidth} canvasHeight={basePlaneHeight}
        inputWidth={Math.round(planeWidth/basePlaneWidth*output.width)}
        inputHeight={Math.round(planeHeight/basePlaneHeight*output.height)}
        outputWidth={output.width} outputHeight={output.height} settings={visualSettings} />
      <mesh ref={meshRef} scale={[planeWidth, planeHeight, 1]}>
        <planeGeometry args={[1, 1]} />
        <Suspense fallback={<VideoFallback />}>
          <VideoMaterial
            viewRef={displayViewRef}
            videoUrl={videoUrl}
            isPlaying={isPlaying}
            cornerRadius={visualSettings.cornerRadius}
            cornerRadiusRatio={outputCornerRadius/inputShortSide}
            temporalMaskRef={nativeMaskRef}
            aspectRatio={planeWidth / planeHeight}
            cleanupWindowCorners={captureMode === 'window'}
            suppressTimeUpdates={linuxNativeOverlayEnabled}
          />
        </Suspense>
        <MaskBoundsEditor viewRef={displayViewRef}/>
        {isLinuxRuntime && (
          <LinuxNativeVideoOverlay
            isPlaying={isPlaying}
            viewRef={displayViewRef}
            onEnabledChange={setLinuxNativeOverlayEnabled}
          />
        )}
      </mesh>
      </group>
      <TemporalWindowPreview original={groupRef} activeRef={temporalWindowActive}
        canvasWidth={basePlaneWidth} canvasHeight={basePlaneHeight} getFrame={temporalFrame}/>
    </>
  );
});
