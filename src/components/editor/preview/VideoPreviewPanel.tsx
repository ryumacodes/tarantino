import { PreviewRedraw } from './PreviewRedraw';
import {CaptionPreviewOverlay} from "./CaptionPreviewOverlay";
import {ShortcutPreviewOverlay} from "./ShortcutPreviewOverlay";
import {CameraPreviewSource} from './CameraPreviewSource';
import {CameraPreviewEffect} from './CameraPreviewEffect';
import {useCameraOpacityTrajectory} from './hooks/useCameraTrajectory';
import {useCameraSilence} from '../../../hooks/useCameraSilence';
import type {PreviewCameraMotion} from '../../../utils/motion/cursorShutter';
import { useShallow } from 'zustand/react/shallow';
import { selectPreviewInputs } from './hooks/videoInputs';
import { timelineDuration } from '../../../utils/timeline/previewTimeline';
import React, { useRef, useState, useEffect, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { EffectComposer } from '@react-three/postprocessing';
import { usePreviewAudioStore } from '../../../stores/previewAudio';
import { useEditorStore } from '../../../stores/editor';
import { VideoViewer } from './VideoViewer';
import { MotionBlurEffect } from './MotionBlurEffect';
import { CursorEffect, type CursorEffectImpl } from './CursorEffect';
import { LoupeEffect } from './LoupeEffect';
import { WebcamPreviewOverlay } from './WebcamPreviewOverlay';
import { ViewControls, PlaybackControls } from './PreviewControls';
import './preview.css';

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

interface VideoPreviewPanelProps {
  isPlaying: boolean;
  onPlayPause: () => void;
  onSeek: (time: number) => void;
  showMouseOverlay: boolean;
}

export const VideoPreviewPanel = React.memo(function VideoPreviewPanel({
  isPlaying,
  onPlayPause,
  onSeek,
  showMouseOverlay
}: VideoPreviewPanelProps) {
  const duration = useEditorStore(timelineDuration);
  const hasLoupe = useEditorStore(state => state.visualSettings.zoomEffectsEnabled !== false && (state.zoomAnalysis?.zoom_blocks.some(block => block.loupe && !block.is_disabled) ?? false));
  const { visualSettings, videoFilePath, displayResolution, outputWidth, outputHeight, hasWebcam } = useEditorStore(useShallow(selectPreviewInputs));

  const gpuCamera=hasWebcam && visualSettings.webcamEnabled !== false;
  const cameraSource=useMemo(()=>new CameraPreviewSource(),[]);
  const cameraSilence=useCameraSilence();
  const cameraOpacity=useCameraOpacityTrajectory(gpuCamera && !cameraSilence.loading,cameraSilence.ranges);
  cameraSource.ready=!cameraOpacity.pending && !cameraOpacity.error && !cameraSilence.loading && !cameraSilence.error;
  cameraSource.opacity=cameraOpacity.frames;
  cameraSource.silence=cameraSilence.ranges;
  useEffect(()=>()=>cameraSource.dispose(),[cameraSource]);
  const isMuted = usePreviewAudioStore(state => state.muted);
  const toggleMute = usePreviewAudioStore(state => state.toggle);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const cursorEffectRef = useRef<CursorEffectImpl | null>(null);
  const cameraMotionRef=useRef<PreviewCameraMotion|null>(null);
  const temporalWindowActive=useRef(false);
  const velocityRef = useRef({ scale: 0, x: 0, y: 0 });

  const videoTransformRef = useRef<VideoTransform>({
    scale: 1,
    offsetX: 0,
    offsetY: 0,
    viewportWidth: 1,
    viewportHeight: 1,
    planeWidth: 1,
    planeHeight: 1
  });

  const sidecarPath = useMemo(() => {
    if (!videoFilePath) return null;
    const directPath = videoFilePath.replace('.mp4', '.mouse.json');
    const fileName = videoFilePath.split('/').pop() || '';
    if (fileName.startsWith('processed_')) {
      const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
      const originalName = fileName.replace('processed_', '');
      return `${dir}/${originalName.replace('.mp4', '.mouse.json')}`;
    }
    return directPath;
  }, [videoFilePath]);

  const handleSeekToEnd = () => {
    onSeek(duration);
  };

  const handleSeekToStart = () => {
    onSeek(0);
  };

  const handleFullscreen = () => {
    if (!isFullscreen && containerRef.current) {
      containerRef.current.requestFullscreen();
      setIsFullscreen(true);
    } else if (document.fullscreenElement) {
      document.exitFullscreen();
      setIsFullscreen(false);
    }
  };

  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(!!document.fullscreenElement);
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, []);

  const videoAspectNum = outputWidth / outputHeight;

  // Calculate frame dimensions that maintain aspect ratio within the container.
  // Pure CSS aspect-ratio + width:100% + max-height:100% breaks when height-constrained.
  const canvasContainerRef = useRef<HTMLDivElement>(null);
  const [frameStyle, setFrameStyle] = useState<React.CSSProperties>({ width: '100%', aspectRatio: `${videoAspectNum}` });

  useEffect(() => {
    const container = canvasContainerRef.current;
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      const { width: cw, height: ch } = entries[0].contentRect;
      if (cw <= 0 || ch <= 0) return;
      if (cw / ch > videoAspectNum) {
        setFrameStyle({ width: Math.floor(ch * videoAspectNum), height: ch });
      } else {
        setFrameStyle({ width: cw, height: Math.floor(cw / videoAspectNum) });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [videoAspectNum]);

  return (
    <div className="video-preview-panel" ref={containerRef}>
      <div className="video-canvas-container" ref={canvasContainerRef}>
        <div className="video-canvas-frame" style={frameStyle}>
          <Canvas
            frameloop={isPlaying ? 'always' : 'demand'}
            camera={{ position: [0, 0, 5], fov: 50 }}
            dpr={[1, 2]}
            gl={{ antialias: true, powerPreference: 'high-performance', alpha: true }}
            style={{
              background: 'var(--editor-bg-secondary)',
              width: '100%',
              height: '100%',
            }}
          >
            <PreviewRedraw />
            <ambientLight intensity={0.5} />
            <pointLight position={[10, 10, 10]} />
            <VideoViewer
              cameraMotionRef={cameraMotionRef}
              showMouseOverlay={showMouseOverlay}
              isPlaying={isPlaying}
              temporalWindowActive={temporalWindowActive}
              velocityRef={velocityRef}
              videoTransformRef={videoTransformRef}
            />

            {(hasLoupe || visualSettings.motionBlurEnabled || (showMouseOverlay && sidecarPath)) && (
              <EffectComposer>
                {visualSettings.motionBlurEnabled ? (
                  <MotionBlurEffect
                    panIntensity={visualSettings.motionBlurPanIntensity}
                    zoomIntensity={visualSettings.motionBlurZoomIntensity}
                    velocityRef={velocityRef}
                    enabled={visualSettings.motionBlurEnabled}
                    temporalWindowActive={temporalWindowActive}
                  />
                ) : <></>}
                {showMouseOverlay && sidecarPath ? (
                  <CursorEffect
                    cameraMotionRef={cameraMotionRef}
                    effectRef={cursorEffectRef}
                    sidecarPath={sidecarPath}
                    videoWidth={displayResolution?.width ?? 1920}
                    videoHeight={displayResolution?.height ?? 1080}
                    visible={showMouseOverlay}
                    videoTransformRef={videoTransformRef}
                  />
                ) : <></>}
                {gpuCamera ? <CameraPreviewEffect source={cameraSource}/> : <></>}
                {hasLoupe ? <LoupeEffect cameraSource={gpuCamera?cameraSource:undefined} videoTransformRef={videoTransformRef} cursorEffectRef={cursorEffectRef} /> : <></>}
              </EffectComposer>
            )}
          </Canvas>
          <ShortcutPreviewOverlay />
          <CaptionPreviewOverlay />
          {gpuCamera && (cameraOpacity.error || cameraSilence.error || cameraOpacity.pending || cameraSilence.loading) &&
            <div role="status" style={{position:'absolute',bottom:12,left:12,zIndex:20,padding:'6px 10px',background:'var(--editor-bg-secondary)',borderRadius:6}}>
              {cameraOpacity.error || cameraSilence.error ? 'Camera preview unavailable. Try reopening the project.' : 'Preparing camera preview…'}
            </div>}


          {hasWebcam && visualSettings.webcamEnabled !== false && videoFilePath && (
            <WebcamPreviewOverlay
              gpuVideo={gpuCamera?cameraSource.setVideo:undefined}
              videoFilePath={videoFilePath}
              x={visualSettings.webcamX ?? 0.895}
              y={visualSettings.webcamY ?? 0.895}
              size={visualSettings.webcamSize}
              shape={visualSettings.webcamShape}
            />
          )}
        </div>

        <div className="video-overlay-controls">
          <ViewControls
            isFullscreen={isFullscreen}
            onFullscreen={handleFullscreen}
          />
        </div>
      </div>

      <PlaybackControls
        isPlaying={isPlaying}
        isMuted={isMuted}
        duration={duration}
        onPlayPause={onPlayPause}
        onSeekBackward={handleSeekToStart}
        onSeekForward={handleSeekToEnd}
        onMuteToggle={toggleMute}
      />
    </div>
  );
});

export default VideoPreviewPanel;
