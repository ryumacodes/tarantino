import React, { useState, useRef, useEffect, useCallback } from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Window } from '@tauri-apps/api/window';
import './styles/globals.css';
import './styles/capture-bar.css';
import './styles/webcam-overlay.css';
import './styles/capture-controls.css';
import './styles/editor-legacy.css';

const bytesToBase64 = (bytes: Uint8Array) => {
  let binary = '';
  const chunkSize = 0x8000;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
  }
  return btoa(binary);
};

interface WebcamDeviceSelection {
  deviceId: string | null;
  deviceName: string | null;
}

const WebcamApp: React.FC = () => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const mediaReadyRef = useRef(false);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const outputPathRef = useRef<string | null>(null);
  const transformRef = useRef({ x: 0.85, y: 0.15, size: 0.15 });
  const [error, setError] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);

  // WebKit and AVFoundation can report different IDs for the same camera.
  const params = new URLSearchParams(window.location.search);
  const [cameraSelection, setCameraSelection] = useState<WebcamDeviceSelection>({
    deviceId: params.get('deviceId'),
    deviceName: params.get('deviceName'),
  });
  const { deviceId, deviceName } = cameraSelection;
  const initialShape = params.get('shape') === 'roundrect' ? 'roundrect' : 'circle';
  const [shape, setShape] = useState<'circle' | 'roundrect'>(initialShape);
  const log = useCallback((level: string, message: string) => {
    console[level === 'error' ? 'error' : 'log'](`[Webcam] ${message}`);
    invoke('webcam_log', { level, message }).catch(() => {});
  }, []);

  const stopCameraStream = useCallback(() => {
    if (videoRef.current) {
      videoRef.current.pause();
      videoRef.current.srcObject = null;
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => {
        try {
          track.stop();
        } catch {}
      });
      streamRef.current = null;
      log('info', 'Camera stream stopped');
    }
  }, [log]);

  // Start camera stream on mount
  useEffect(() => {
    let cancelled = false;
    let started = false;
    let fallbackTimer: ReturnType<typeof setTimeout> | undefined;

    const startCamera = async () => {
      if (started || cancelled) return;
      started = true;

      try {
        setError(null);
        if (!navigator.mediaDevices?.getUserMedia) {
          throw new Error('navigator.mediaDevices.getUserMedia is unavailable');
        }

        log('info', `Page context href=${window.location.href} secure=${window.isSecureContext} visibility=${document.visibilityState}`);
        if (navigator.permissions?.query) {
          try {
            const cameraPermission = await navigator.permissions.query({ name: 'camera' as PermissionName });
            log('info', `Permissions API camera state=${cameraPermission.state}`);
          } catch (permissionError) {
            const error = permissionError as DOMException;
            log('info', `Permissions API camera query failed: ${error.name ?? 'Error'}: ${error.message ?? String(permissionError)}`);
          }
        }

        let webkitDeviceId: string | null = null;
        if (deviceId) {
          const findSelectedDevice = (devices: MediaDeviceInfo[]) => devices.find((device) =>
            device.kind === 'videoinput' && (
              device.deviceId === deviceId
              || (!!deviceName && device.label.trim().toLowerCase() === deviceName.trim().toLowerCase())
            )
          );

          let selectedDevice = findSelectedDevice(await navigator.mediaDevices.enumerateDevices());
          if (!selectedDevice && deviceName) {
            // Labels may be unavailable until camera permission is granted.
            const permissionStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
            permissionStream.getTracks().forEach((track) => track.stop());
            selectedDevice = findSelectedDevice(await navigator.mediaDevices.enumerateDevices());
          }

          if (!selectedDevice) {
            throw new Error(`Selected camera is unavailable: ${deviceName ?? deviceId}`);
          }
          webkitDeviceId = selectedDevice.deviceId;
        }

        const constraints: MediaStreamConstraints = {
          video: {
            ...(webkitDeviceId ? { deviceId: { exact: webkitDeviceId } } : {}),
            width: { ideal: 1280 },
            height: { ideal: 720 },
            frameRate: { ideal: 30 },
          },
          audio: false,
        };
        log('info', `Requesting camera stream device=${deviceName ?? deviceId ?? 'default'}`);
        const stream = await navigator.mediaDevices.getUserMedia(constraints);
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
        }
        const track = stream.getVideoTracks()[0];
        const settings = track?.getSettings?.();
        log('info', `Camera stream started ${settings?.width ?? '?'}x${settings?.height ?? '?'} @ ${settings?.frameRate ?? '?'}fps`);
      } catch (err) {
        const error = err as DOMException;
        log('error', `Failed to start camera: ${error.name ?? 'Error'}: ${error.message ?? String(err)}`);
        setError('Camera access denied');
      }
    };

    const removeReadyListener = listen('webcam:ready-to-start', () => {
      log('info', 'Backend media delegate is ready');
      mediaReadyRef.current = true;
      startCamera();
    });

    if (mediaReadyRef.current) {
      startCamera();
    } else {
      fallbackTimer = setTimeout(() => {
        log('info', 'Camera readiness fallback fired');
        mediaReadyRef.current = true;
        startCamera();
      }, 2000);
    }

    return () => {
      cancelled = true;
      if (fallbackTimer) clearTimeout(fallbackTimer);
      removeReadyListener.then((fn) => fn()).catch(() => {});
      stopCameraStream();
    };
  }, [deviceId, deviceName, log, stopCameraStream]);

  const startRecording = useCallback(() => {
    const stream = streamRef.current;
    if (!stream || isRecording) {
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
        if (blob.size > 0) {
          try {
            const buf = await blob.arrayBuffer();
            const dataBase64 = bytesToBase64(new Uint8Array(buf));
            stopCameraStream();
            await invoke('save_webcam_recording', {
              dataBase64,
              position: { x: transformRef.current.x, y: transformRef.current.y },
              size: transformRef.current.size,
              shape,
              outputPath: outputPathRef.current,
            });
          } catch (err) {
            console.error('[Webcam] Failed to save recording:', err);
          }
        }
        stopCameraStream();
        // Close ourselves after saving
        Window.getCurrent().close().catch(() => {});
      };
      recorder.start(1000);
      mediaRecorderRef.current = recorder;
      setIsRecording(true);
    } catch (err) {
      console.error('[Webcam] Failed to start recording:', err);
    }
  }, [isRecording, shape, stopCameraStream]);

  const stopRecording = useCallback(() => {
    if (
      mediaRecorderRef.current &&
      mediaRecorderRef.current.state !== 'inactive'
    ) {
      mediaRecorderRef.current.stop();
      mediaRecorderRef.current = null;
      setIsRecording(false);
    } else {
      stopCameraStream();
    }
  }, [stopCameraStream]);

  // Listen for recording lifecycle events
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<any>('recording:started', (event) => {
      const payload = event.payload;
      if (payload && typeof payload === 'object' && typeof payload.output_path === 'string') {
        outputPathRef.current = payload.output_path;
        const webcamX = Number(payload.webcam_x);
        const webcamY = Number(payload.webcam_y);
        const webcamSize = Number(payload.webcam_size);
        transformRef.current = {
          x: Number.isFinite(webcamX) ? webcamX : transformRef.current.x,
          y: Number.isFinite(webcamY) ? webcamY : transformRef.current.y,
          size: Number.isFinite(webcamSize) ? webcamSize : transformRef.current.size,
        };
      } else if (typeof payload === 'string') {
        try {
          const parsed = JSON.parse(payload);
          if (typeof parsed.output_path === 'string') {
            outputPathRef.current = parsed.output_path;
          }
          const webcamX = Number(parsed.webcam_x);
          const webcamY = Number(parsed.webcam_y);
          const webcamSize = Number(parsed.webcam_size);
          transformRef.current = {
            x: Number.isFinite(webcamX) ? webcamX : transformRef.current.x,
            y: Number.isFinite(webcamY) ? webcamY : transformRef.current.y,
            size: Number.isFinite(webcamSize) ? webcamSize : transformRef.current.size,
          };
        } catch {
          // Older start events only carried timer data; keep the last output path.
        }
      }
      startRecording();
    }).then((fn) => unlisteners.push(fn));

    // Backend sends this BEFORE closing the window, giving us time to save
    listen('webcam:stop', () => {
      if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
        // Stop the recorder — the onstop handler will save and close the window
        mediaRecorderRef.current.stop();
        mediaRecorderRef.current = null;
        setIsRecording(false);
      } else {
        // No recording was active — just close the window
        stopCameraStream();
        Window.getCurrent().close().catch(() => {});
      }
    }).then((fn) => unlisteners.push(fn));

    // Fallback: explicit close request
    listen('webcam:close', () => {
      stopRecording();
      stopCameraStream();
      Window.getCurrent().close().catch(() => {});
    }).then((fn) => unlisteners.push(fn));

    listen<string>('webcam:set-shape', (event) => {
      const nextShape = event.payload === 'roundrect' ? 'roundrect' : 'circle';
      setShape(nextShape);
    }).then((fn) => unlisteners.push(fn));

    listen<WebcamDeviceSelection>('webcam:set-device', (event) => {
      setCameraSelection(event.payload);
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [startRecording, stopCameraStream, stopRecording]);

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
        width: '100vmin',
        height: '100vmin',
        maxWidth: '100%',
        maxHeight: '100%',
        aspectRatio: '1 / 1',
        borderRadius: shape === 'roundrect' ? '18%' : '50%',
        overflow: 'hidden',
        cursor: 'grab',
        background: '#000',
        position: 'relative',
        margin: 'auto',
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
