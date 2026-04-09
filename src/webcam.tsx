import React, { useState, useRef, useEffect, useCallback } from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Window } from '@tauri-apps/api/window';
import './styles/globals.css';

const WebcamApp: React.FC = () => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);

  // Get device ID from URL params
  const params = new URLSearchParams(window.location.search);
  const deviceId = params.get('deviceId');

  // Start camera stream on mount
  useEffect(() => {
    const startCamera = async () => {
      try {
        const constraints: MediaStreamConstraints = {
          video: {
            ...(deviceId ? { deviceId } : {}),
            width: { ideal: 1280 },
            height: { ideal: 720 },
            frameRate: { ideal: 30 },
          },
          audio: false,
        };
        const stream = await navigator.mediaDevices.getUserMedia(constraints);
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
        }
        console.log('[Webcam] Camera stream started');
      } catch (err) {
        console.error('[Webcam] Failed to start camera:', err);
        setError('Camera access denied');
      }
    };
    startCamera();

    return () => {
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((t) => t.stop());
      }
    };
  }, [deviceId]);

  const startRecording = useCallback(() => {
    const stream = streamRef.current;
    if (!stream || isRecording) {
      console.log('[Webcam] Cannot start recording — no stream or already recording');
      return;
    }
    try {
      chunksRef.current = [];
      const recorder = new MediaRecorder(stream, {
        mimeType: 'video/webm; codecs=vp9',
      });
      recorder.ondataavailable = (e) => {
        if (e.data.size > 0) chunksRef.current.push(e.data);
      };
      recorder.onstop = async () => {
        const blob = new Blob(chunksRef.current, { type: 'video/webm' });
        console.log('[Webcam] Recording stopped, blob size:', blob.size);
        if (blob.size > 0) {
          try {
            const buf = await blob.arrayBuffer();
            await invoke('save_webcam_recording', {
              data: Array.from(new Uint8Array(buf)),
              position: { x: 0.85, y: 0.85 },
              size: 0.15,
              shape: 'circle',
            });
            console.log('[Webcam] Recording saved to sidecar');
          } catch (err) {
            console.error('[Webcam] Failed to save recording:', err);
          }
        }
        // Close ourselves after saving
        Window.getCurrent().close().catch(() => {});
      };
      recorder.start(1000);
      mediaRecorderRef.current = recorder;
      setIsRecording(true);
      console.log('[Webcam] MediaRecorder started');
    } catch (err) {
      console.error('[Webcam] Failed to start recording:', err);
    }
  }, [isRecording]);

  const stopRecording = useCallback(() => {
    if (
      mediaRecorderRef.current &&
      mediaRecorderRef.current.state !== 'inactive'
    ) {
      mediaRecorderRef.current.stop();
      mediaRecorderRef.current = null;
      setIsRecording(false);
    }
  }, []);

  // Listen for recording lifecycle events
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen('recording:started', () => {
      console.log('[Webcam] Recording started event — starting capture (window already hidden by backend)');
      startRecording();
    }).then((fn) => unlisteners.push(fn));

    // Backend sends this BEFORE closing the window, giving us time to save
    listen('webcam:stop', () => {
      console.log('[Webcam] Stop event — stopping capture and saving');
      if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
        // Stop the recorder — the onstop handler will save and close the window
        mediaRecorderRef.current.stop();
        mediaRecorderRef.current = null;
        setIsRecording(false);
      } else {
        // No recording was active — just close the window
        console.log('[Webcam] No active recording, closing window');
        Window.getCurrent().close().catch(() => {});
      }
    }).then((fn) => unlisteners.push(fn));

    // Fallback: explicit close request
    listen('webcam:close', () => {
      stopRecording();
      Window.getCurrent().close().catch(() => {});
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [startRecording, stopRecording]);

  // Enable dragging
  const handleDragStart = async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      await Window.getCurrent().startDragging();
    } catch {}
  };

  return (
    <div
      onMouseDown={handleDragStart}
      style={{
        width: '100%',
        height: '100%',
        borderRadius: '50%',
        overflow: 'hidden',
        cursor: 'grab',
        background: '#000',
        position: 'relative',
      }}
    >
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        style={{
          width: '100%',
          height: '100%',
          objectFit: 'cover',
          transform: 'scaleX(-1)',
        }}
      />
      {isRecording && (
        <div
          style={{
            position: 'absolute',
            top: 8,
            right: 8,
            width: 10,
            height: 10,
            borderRadius: '50%',
            background: '#ff4747',
            boxShadow: '0 0 6px rgba(255,71,71,0.6)',
          }}
        />
      )}
      {error && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: '#ff6b6b',
            fontSize: 12,
            textAlign: 'center',
            padding: 12,
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
};

ReactDOM.createRoot(document.getElementById('root')!).render(<WebcamApp />);
