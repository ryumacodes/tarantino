import React, { useRef, useState, useEffect, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { EffectComposer } from '@react-three/postprocessing';
import { useEditorStore } from '../../../stores/editor';
import { VideoViewer } from './VideoViewer';
import { MotionBlurEffect } from './MotionBlurEffect';
import { CursorEffect } from './CursorEffect';
import { WebcamPreviewOverlay } from './WebcamPreviewOverlay';
import { ViewControls, PlaybackControls } from './PreviewControls';
import './preview.css';

interface VideoTransform {
  scale: number;
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

export const VideoPreviewPanel: React.FC<VideoPreviewPanelProps> = ({
  isPlaying,
  onPlayPause,
  onSeek,
  showMouseOverlay
}) => {
  const { duration, currentTime, visualSettings, videoFilePath, displayResolution, captureMode, hasWebcam } = useEditorStore();

  const [isMuted, setIsMuted] = useState(true);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

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

  // Focus window recordings keep the source aspect. Desktop window recordings
  // use the export canvas aspect ratio so background staging is visible.
  const ASPECT_RATIOS_NUM: Record<string, number> = {
    '16:9': 16/9, '9:16': 9/16, '4:3': 4/3,
    '1:1': 1, '21:9': 21/9,
  };
  const sourceAspectNum = displayResolution
    ? displayResolution.width / displayResolution.height
    : 16 / 9;
  const isWindowFocus = captureMode === 'window' && visualSettings.windowLayoutMode === 'focus';
  const videoAspectNum = isWindowFocus
    ? sourceAspectNum
    : captureMode === 'window'
      ? (ASPECT_RATIOS_NUM[visualSettings.aspectRatio] || sourceAspectNum)
    : displayResolution
      ? displayResolution.width / displayResolution.height
      : 16 / 9;

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
            camera={{ position: [0, 0, 5], fov: 50 }}
            dpr={[1, 2]}
            gl={{ antialias: true, powerPreference: 'high-performance', alpha: true }}
            style={{
              background: 'var(--editor-bg-secondary)',
              width: '100%',
              height: '100%',
            }}
          >
            <ambientLight intensity={0.5} />
            <pointLight position={[10, 10, 10]} />
            <VideoViewer
              showMouseOverlay={showMouseOverlay}
              isPlaying={isPlaying}
              velocityRef={velocityRef}
              videoTransformRef={videoTransformRef}
            />

            {(visualSettings.motionBlurEnabled || (showMouseOverlay && sidecarPath)) && (
              <EffectComposer>
                {visualSettings.motionBlurEnabled ? (
                  <MotionBlurEffect
                    panIntensity={visualSettings.motionBlurPanIntensity}
                    zoomIntensity={visualSettings.motionBlurZoomIntensity}
                    velocityRef={velocityRef}
                    enabled={visualSettings.motionBlurEnabled}
                  />
                ) : <></>}
                {showMouseOverlay && sidecarPath ? (
                  <CursorEffect
                    sidecarPath={sidecarPath}
                    videoWidth={displayResolution?.width ?? 1920}
                    videoHeight={displayResolution?.height ?? 1080}
                    visible={showMouseOverlay}
                    videoTransformRef={videoTransformRef}
                  />
                ) : <></>}
              </EffectComposer>
            )}
          </Canvas>

          {hasWebcam && videoFilePath && (
            <WebcamPreviewOverlay
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
        currentTime={currentTime}
        duration={duration}
        onPlayPause={onPlayPause}
        onSeekBackward={handleSeekToStart}
        onSeekForward={handleSeekToEnd}
        onMuteToggle={() => setIsMuted(!isMuted)}
      />
    </div>
  );
};

export default VideoPreviewPanel;
