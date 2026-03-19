import React, { useState, useRef, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Circle, Square, Camera, CameraOff, Video } from 'lucide-react';
import { useRecordingStore } from '../stores/recording';
import { cn } from '../utils/cn';

interface Position {
  x: number;
  y: number;
}

interface WebcamOverlayProps {
  onRecordingDataReady?: (blob: Blob) => void;
}

const WebcamOverlay: React.FC<WebcamOverlayProps> = ({ onRecordingDataReady }) => {
  const [position, setPosition] = useState<Position>({ x: 0.86, y: 0.14 });
  const [size, setSize] = useState(0.12);
  const [shape, setShape] = useState<'circle' | 'roundrect'>('circle');
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [cameraEnabled, setCameraEnabled] = useState(false);
  const [cameraStream, setCameraStream] = useState<MediaStream | null>(null);
  const [availableCameras, setAvailableCameras] = useState<MediaDeviceInfo[]>([]);
  const [selectedCameraId, setSelectedCameraId] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const recordedChunksRef = useRef<Blob[]>([]);

  const { state } = useRecordingStore();
  const isVisible = cameraEnabled;
  const shouldRecord = state === 'recording';

  // Enumerate available cameras
  useEffect(() => {
    const enumerateCameras = async () => {
      try {
        const devices = await navigator.mediaDevices.enumerateDevices();
        const cameras = devices.filter(device => device.kind === 'videoinput');
        setAvailableCameras(cameras);
        if (cameras.length > 0 && !selectedCameraId) {
          setSelectedCameraId(cameras[0].deviceId);
        }
      } catch (error) {
        console.error('Failed to enumerate cameras:', error);
        setCameraError('Failed to list cameras');
      }
    };
    enumerateCameras();
  }, []);

  // Initialize camera stream when enabled
  useEffect(() => {
    if (!cameraEnabled || !selectedCameraId) {
      if (cameraStream) {
        cameraStream.getTracks().forEach(track => track.stop());
        setCameraStream(null);
      }
      return;
    }

    const initCamera = async () => {
      try {
        setCameraError(null);
        const stream = await navigator.mediaDevices.getUserMedia({
          video: {
            deviceId: selectedCameraId,
            width: { ideal: 1280 },
            height: { ideal: 720 },
            frameRate: { ideal: 30 }
          },
          audio: false // We capture audio separately via system
        });
        setCameraStream(stream);

        if (videoRef.current) {
          videoRef.current.srcObject = stream;
        }
        console.log('Camera stream initialized');
      } catch (error) {
        console.error('Failed to initialize camera:', error);
        setCameraError('Camera access denied');
        setCameraEnabled(false);
      }
    };

    initCamera();

    return () => {
      if (cameraStream) {
        cameraStream.getTracks().forEach(track => track.stop());
      }
    };
  }, [cameraEnabled, selectedCameraId]);

  // Sync video element with stream
  useEffect(() => {
    if (videoRef.current && cameraStream) {
      videoRef.current.srcObject = cameraStream;
    }
  }, [cameraStream]);

  // Start/stop recording based on screen recording state
  useEffect(() => {
    if (shouldRecord && cameraStream && cameraEnabled && !isRecording) {
      startRecording();
    } else if (!shouldRecord && isRecording) {
      stopRecording();
    }
  }, [shouldRecord, cameraStream, cameraEnabled, isRecording]);

  const startRecording = useCallback(() => {
    if (!cameraStream) return;

    try {
      recordedChunksRef.current = [];
      const options = { mimeType: 'video/webm; codecs=vp9' };
      const recorder = new MediaRecorder(cameraStream, options);

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          recordedChunksRef.current.push(event.data);
        }
      };

      recorder.onstop = () => {
        const blob = new Blob(recordedChunksRef.current, { type: 'video/webm' });
        console.log('Webcam recording stopped, blob size:', blob.size);
        if (onRecordingDataReady) {
          onRecordingDataReady(blob);
        }
        // Save webcam recording to sidecar folder
        saveWebcamRecording(blob);
      };

      recorder.start(1000); // Collect data every second
      mediaRecorderRef.current = recorder;
      setIsRecording(true);
      console.log('Webcam recording started');
    } catch (error) {
      console.error('Failed to start webcam recording:', error);
    }
  }, [cameraStream, onRecordingDataReady]);

  const stopRecording = useCallback(() => {
    if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
      mediaRecorderRef.current.stop();
      setIsRecording(false);
    }
  }, []);

  const saveWebcamRecording = async (blob: Blob) => {
    try {
      // Convert blob to array buffer for Tauri
      const arrayBuffer = await blob.arrayBuffer();
      const uint8Array = new Uint8Array(arrayBuffer);

      // Save to sidecar folder
      await invoke('save_webcam_recording', {
        data: Array.from(uint8Array),
        position: { x: position.x, y: position.y },
        size: size,
        shape: shape
      });
      console.log('Webcam recording saved to sidecar');
    } catch (error) {
      console.error('Failed to save webcam recording:', error);
    }
  };

  const toggleCamera = useCallback(() => {
    setCameraEnabled(!cameraEnabled);
  }, [cameraEnabled]);

  const handleDragStart = (e: React.MouseEvent) => {
    if (isResizing) return;
    setIsDragging(true);
    e.preventDefault();
  };

  const handleDrag = (e: MouseEvent) => {
    if (!isDragging || !containerRef.current) return;

    const rect = containerRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));

    // Snap to edges/corners
    const snapThreshold = 0.05;
    const snapPositions = [0, 0.5, 1];
    
    const snappedX = snapPositions.reduce((prev, curr) => 
      Math.abs(curr - x) < Math.abs(prev - x) && Math.abs(curr - x) < snapThreshold ? curr : prev, x
    );
    const snappedY = snapPositions.reduce((prev, curr) =>
      Math.abs(curr - y) < Math.abs(prev - y) && Math.abs(curr - y) < snapThreshold ? curr : prev, y
    );

    setPosition({ x: snappedX, y: snappedY });
    updateTransform(snappedX, snappedY, size, shape);
  };

  const handleDragEnd = () => {
    setIsDragging(false);
  };

  const handleResize = (delta: number) => {
    const newSize = Math.max(0.08, Math.min(0.25, size + delta * 0.01));
    setSize(newSize);
    updateTransform(position.x, position.y, newSize, shape);
  };

  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    handleResize(-e.deltaY * 0.001);
  };

  const toggleShape = () => {
    const newShape = shape === 'circle' ? 'roundrect' : 'circle';
    setShape(newShape);
    updateTransform(position.x, position.y, size, newShape);
  };

  const updateTransform = async (x: number, y: number, size: number, shape: 'circle' | 'roundrect') => {
    await invoke('webcam_set_transform', {
      xNorm: x,
      yNorm: y,
      sizeNorm: size,
      shape
    });
  };

  useEffect(() => {
    if (isDragging) {
      document.addEventListener('mousemove', handleDrag);
      document.addEventListener('mouseup', handleDragEnd);
      return () => {
        document.removeEventListener('mousemove', handleDrag);
        document.removeEventListener('mouseup', handleDragEnd);
      };
    }
  }, [isDragging]);

  const overlaySize = Math.min(window.innerWidth, window.innerHeight) * size;
  const overlayStyle = {
    width: `${overlaySize}px`,
    height: `${overlaySize}px`,
    left: `${position.x * 100}%`,
    top: `${position.y * 100}%`,
    transform: 'translate(-50%, -50%)',
  };

  return (
    <div ref={containerRef} className="webcam-overlay-container">
      {/* Camera toggle button - always visible */}
      <motion.button
        className={cn('webcam-toggle-button', { active: cameraEnabled, recording: isRecording })}
        onClick={toggleCamera}
        whileHover={{ scale: 1.1 }}
        whileTap={{ scale: 0.95 }}
        title={cameraEnabled ? 'Disable camera' : 'Enable camera'}
      >
        {cameraEnabled ? <Camera size={18} /> : <CameraOff size={18} />}
        {isRecording && (
          <span className="webcam-toggle-button__recording-indicator" />
        )}
      </motion.button>

      {/* Camera overlay - only visible when camera is enabled */}
      <AnimatePresence>
        {isVisible && cameraStream && (
          <motion.div
            ref={overlayRef}
            className={cn('webcam-overlay', shape, { dragging: isDragging, recording: isRecording })}
            style={overlayStyle}
            onMouseDown={handleDragStart}
            onWheel={handleWheel}
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{ duration: 0.2 }}
          >
            <div className="webcam-overlay__content">
              <video
                ref={videoRef}
                className="webcam-overlay__video"
                autoPlay
                muted
                playsInline
              />
              {/* Recording indicator */}
              {isRecording && (
                <div className="webcam-overlay__recording-badge">
                  <Video size={10} />
                  <span>REC</span>
                </div>
              )}
              <div className="webcam-overlay__controls">
                <button
                  className="webcam-overlay__shape-toggle"
                  onClick={toggleShape}
                  onMouseDown={(e) => e.stopPropagation()}
                  title={shape === 'circle' ? 'Switch to rectangle' : 'Switch to circle'}
                >
                  {shape === 'circle' ? <Square size={14} /> : <Circle size={14} />}
                </button>
              </div>
              <div
                className="webcam-overlay__resize-handle"
                onMouseDown={(e) => {
                  e.stopPropagation();
                  setIsResizing(true);
                }}
                onMouseUp={() => setIsResizing(false)}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Drag guides */}
      {(isDragging || isResizing) && (
        <div className="webcam-overlay__guides">
          <div className="webcam-overlay__guide webcam-overlay__guide--horizontal" style={{ top: '33.33%' }} />
          <div className="webcam-overlay__guide webcam-overlay__guide--horizontal" style={{ top: '66.66%' }} />
          <div className="webcam-overlay__guide webcam-overlay__guide--vertical" style={{ left: '33.33%' }} />
          <div className="webcam-overlay__guide webcam-overlay__guide--vertical" style={{ left: '66.66%' }} />
        </div>
      )}

      {/* Error message */}
      {cameraError && (
        <motion.div
          className="webcam-overlay__error"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 10 }}
        >
          {cameraError}
        </motion.div>
      )}
    </div>
  );
};

// Export webcam state for external access
export const useWebcamState = () => {
  const [enabled, setEnabled] = useState(false);
  return { enabled, setEnabled };
};

export default WebcamOverlay;