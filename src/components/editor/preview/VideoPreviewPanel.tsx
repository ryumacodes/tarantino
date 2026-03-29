import React, { useRef, useState, useEffect, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { EffectComposer } from '@react-three/postprocessing';
import { useEditorStore } from '../../../stores/editor';
import { VideoViewer } from './VideoViewer';
import { MotionBlurEffect } from './MotionBlurEffect';
import { CursorEffect } from './CursorEffect';
import { ZoomControls, ViewControls, PlaybackControls } from './PreviewControls';
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
  const { duration, currentTime, visualSettings, videoFilePath, displayResolution, captureMode } = useEditorStore();

  console.log('%c[VideoPreviewPanel] RENDERING', 'background: #ff0000; color: white; font-size: 16px;', {
    videoFilePath,
    displayResolution,
    showMouseOverlay,
  });

  const [isMuted, setIsMuted] = useState(true);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [zoom, setZoom] = useState(1);
  const containerRef = useRef<HTMLDivElement>(null);

  // Velocity ref for motion blur
  const velocityRef = useRef({ scale: 0, x: 0, y: 0 });

  // Video transform ref for cursor overlay synchronization
  const videoTransformRef = useRef<VideoTransform>({
    scale: 1,
    offsetX: 0,
    offsetY: 0,
    viewportWidth: 1,
    viewportHeight: 1,
    planeWidth: 1,
    planeHeight: 1
  });

  // Derive sidecar path from video file path
  // For processed_ files, also check the original recording path
  const sidecarPath = useMemo(() => {
    if (!videoFilePath) return null;
    const directPath = videoFilePath.replace('.mp4', '.mouse.json');
    // If video is processed_recording_*, the mouse.json may be at recording_* instead
    const fileName = videoFilePath.split('/').pop() || '';
    if (fileName.startsWith('processed_')) {
      const dir = videoFilePath.substring(0, videoFilePath.lastIndexOf('/'));
      const originalName = fileName.replace('processed_', '');
      return `${dir}/${originalName.replace('.mp4', '.mouse.json')}`;
    }
    return directPath;
  }, [videoFilePath]);

  useEffect(() => {
    console.log('[VideoPreviewPanel] videoFilePath:', videoFilePath);
    console.log('[VideoPreviewPanel] sidecarPath:', sidecarPath);
    console.log('[VideoPreviewPanel] showMouseOverlay:', showMouseOverlay);
    console.log('[VideoPreviewPanel] displayResolution:', displayResolution);
    console.log('[VideoPreviewPanel] Will render CursorEffect:', !!(showMouseOverlay && sidecarPath));
  }, [videoFilePath, sidecarPath, showMouseOverlay, displayResolution]);

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

  const handleZoomIn = () => {
    setZoom(prev => Math.min(prev * 1.2, 4));
  };

  const handleZoomOut = () => {
    setZoom(prev => Math.max(prev / 1.2, 0.5));
  };

  const handleZoomReset = () => {
    setZoom(1);
  };

  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setZoom(prev => Math.min(Math.max(prev * delta, 0.5), 4));
  };

  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(!!document.fullscreenElement);
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, []);

  // For window recordings, use the export canvas aspect ratio (e.g. 16:9)
  // For display recordings, use the display's native aspect ratio
  const ASPECT_RATIOS_NUM: Record<string, number> = {
    '16:9': 16/9, '9:16': 9/16, '4:3': 4/3,
    '1:1': 1, '21:9': 21/9,
  };
  const videoAspectNum = captureMode === 'window'
    ? (ASPECT_RATIOS_NUM[visualSettings.aspectRatio] || 16/9)
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
        // Container wider than target — constrain by height
        setFrameStyle({ width: Math.floor(ch * videoAspectNum), height: ch });
      } else {
        // Container taller — constrain by width
        setFrameStyle({ width: cw, height: Math.floor(cw / videoAspectNum) });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [videoAspectNum]);

  return (
    <div className="video-preview-panel" ref={containerRef}>
      {/* Video Canvas */}
      <div className="video-canvas-container" ref={canvasContainerRef} onWheel={handleWheel}>
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
              previewZoom={zoom}
            />
            <OrbitControls enablePan={zoom > 1} enableZoom={false} enableRotate={false} />

            {/* Post-processing effects: motion blur + cursor */}
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
        </div>

        {/* Video Overlay Controls */}
        <div className="video-overlay-controls">
          <ZoomControls
            zoom={zoom}
            onZoomIn={handleZoomIn}
            onZoomOut={handleZoomOut}
            onZoomReset={handleZoomReset}
          />
          <ViewControls
            isFullscreen={isFullscreen}
            onFullscreen={handleFullscreen}
          />
        </div>
      </div>

      {/* Playback Controls */}
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
