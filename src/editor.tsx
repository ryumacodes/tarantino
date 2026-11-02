import { projectLoadError } from './utils/projects/loadError';
import RelinkRecording from './components/editor/RelinkRecording';
import { saveProject } from './utils/projects/save';
import { saveBeforeRecordingHandoff } from './utils/projects/handoff';
import { applyDefaultPreset } from './utils/presets/defaultPreset';
import { restoredProjectState } from './utils/projects/restore';
import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import EditorView from './components/EditorView';
import ProjectLibrary from './components/editor/ProjectLibrary';
import { parseProjectDocument } from './utils/projects/document';
import { useEditorStore } from './stores/editor';
import './styles/globals.css';
import './styles/capture-bar.css';
import './styles/webcam-overlay.css';
import './styles/capture-controls.css';
import './styles/editor-legacy.css';
import './styles/dracula-theme.css';
import './styles/shortcuts.css';

class ErrorBoundary extends React.Component<{ children: React.ReactNode }, { hasError: boolean, error?: Error }> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error) {
    console.error('Error caught by boundary:', error);
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Error boundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          padding: '20px',
          background: '#1C1B19',
          color: '#FCE8C3',
          height: '100vh',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          alignItems: 'center'
        }}>
          <h1>Something went wrong</h1>
          <details>
            <summary>Error details</summary>
            <pre style={{ color: '#EF2F27', marginTop: '10px' }}>
              {this.state.error?.toString()}
              {'\n'}
              {this.state.error?.stack}
            </pre>
          </details>
        </div>
      );
    }

    return this.props.children;
  }
}

let setupProcessingEventListeners: () => void;
type RecordingReady = {
  path: string;
  has_mic?: boolean;
  has_system_audio?: boolean;
  has_webcam?: boolean;
  webcam_shape?: string;
  webcam_x?: number;
  webcam_y?: number;
  webcam_size?: number;
};
type WebcamShape = 'circle' | 'roundrect';

const normalizeWebcamShape = (shape: string | null | undefined): WebcamShape => {
  return shape === 'roundrect' || shape === 'rounded' || shape === 'square' ? 'roundrect' : 'circle';
};

const readWebcamTransform = (source: Record<string, unknown> | URLSearchParams | null | undefined) => {
  const readNumber = (key: string, fallback: number) => {
    const raw = source instanceof URLSearchParams ? source.get(key) : source?.[key];
    if (raw == null || raw === '') return fallback;
    const value = typeof raw === 'number' ? raw : Number(raw);
    return Number.isFinite(value) ? value : fallback;
  };

  return {
    webcamX: readNumber('webcam_x', 0.895),
    webcamY: readNumber('webcam_y', 0.895),
    webcamSize: readNumber('webcam_size', 0.15),
  };
};

function EditorShell() {
  const [showProjectLibrary, setShowProjectLibrary] = useState(false);
  const [mediaPath, setMediaPath] = useState<string | null>(null);
  const [presetWarning,setPresetWarning]=useState<string|null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [processingStatus, setProcessingStatus] = useState<string>('Loading...');

  const { initializeEditor, updateVisualSettings } = useEditorStore();

  useEffect(() => {
    let handingOff = false;
    const receive = (event: Event) => {
      const destination = (event as CustomEvent<string>).detail;
      if (typeof destination !== 'string') return;
      event.preventDefault();
      if (handingOff) return;
      handingOff = true;
      setIsLoading(true);
      setProcessingStatus('Saving current project…');
      useEditorStore.getState().setIsPlaying(false);
      void saveBeforeRecordingHandoff(useEditorStore.getState(),
        new URLSearchParams(window.location.search).get('project'), destination,
        saveProject,
        url => window.location.assign(url),
      ).catch(error => {
        handingOff = false;
        setIsLoading(false);
        setPresetWarning(`Could not save the current project: ${String(error)}. Your new recording is available in Open project.`);
      });
    };
    window.addEventListener('tarantino-recording-handoff', receive);
    return () => window.removeEventListener('tarantino-recording-handoff', receive);
  }, []);

  setupProcessingEventListeners = () => {
    listen<string>('processing-status', (event) => {
      setProcessingStatus(event.payload);
    });

    listen<string | RecordingReady>('recording-ready', async (event) => {
      const payload = event.payload;
      const finalPath = typeof payload === 'string' ? payload : payload.path;
      const hasMic = typeof payload === 'object' ? !!payload.has_mic : false;
      const hasSystemAudio = typeof payload === 'object' ? !!payload.has_system_audio : false;
      const hasWebcamFromPayload = typeof payload === 'object' ? !!payload.has_webcam : false;
      const webcamShapeFromPayload = typeof payload === 'object'
        ? normalizeWebcamShape(payload.webcam_shape)
        : normalizeWebcamShape(null);
      const webcamTransformFromPayload = typeof payload === 'object'
        ? readWebcamTransform(payload)
        : null;
      const urlParams = new URLSearchParams(window.location.search);
      const urlHasWebcam = urlParams.get('webcam') === 'true';
      const webcamShapeFromUrl = normalizeWebcamShape(urlParams.get('webcam_shape'));
      const webcamTransformFromUrl = readWebcamTransform(urlParams);
      const webcamEnabled = hasWebcamFromPayload || urlHasWebcam;
      const webcamShape = hasWebcamFromPayload ? webcamShapeFromPayload : webcamShapeFromUrl;
      const webcamTransform = hasWebcamFromPayload && webcamTransformFromPayload
        ? webcamTransformFromPayload
        : webcamTransformFromUrl;

      setMediaPath(finalPath);
      setProcessingStatus('Ready!');
      setIsLoading(true);

      initializeEditorWithPath(finalPath, hasMic, hasSystemAudio, webcamEnabled, webcamShape, webcamTransform);
    });
  };

  const initializeEditorWithPath = async (
    path: string,
    hasMic = false,
    hasSystemAudio = false,
    hasWebcam = false,
    webcamShape: WebcamShape = 'circle',
    webcamTransform = { webcamX: 0.895, webcamY: 0.895, webcamSize: 0.15 }
  ) => {
    try {
      const videoInfo = await invoke<any>('get_video_metadata', { filePath: path });

      let duration = 10000;
      if (videoInfo.duration_ms) {
        duration = videoInfo.duration_ms;
      } else if (videoInfo.duration) {
        duration = videoInfo.duration * 1000;
      }

      await initializeEditor(path, duration, hasWebcam, hasMic, hasSystemAudio, videoInfo.width ?? null, videoInfo.height ?? null);
      if (hasWebcam) {
        updateVisualSettings({ webcamShape, ...webcamTransform });
      }

      try {await applyDefaultPreset();setPresetWarning(null);}
      catch {setPresetWarning('The default style could not be loaded. Your recording is ready; open Style presets to reload the library.');}
      setIsLoading(false);
    } catch (err) {
      console.error('Error initializing editor with final path:', err);

      let errorMessage = 'Unknown error occurred';
      const errorStr = String(err);

      if (errorStr.includes('Video file not found')) {
        errorMessage = 'Recording failed - no video file was created. This may be due to screen recording permissions or FFmpeg issues. Please try recording again.';
      } else if (errorStr.includes('permission')) {
        errorMessage = 'Permission denied - please check screen recording permissions in System Preferences > Security & Privacy > Screen Recording.';
      } else if (errorStr.includes('FFmpeg')) {
        errorMessage = 'FFmpeg error occurred during recording. Please try recording again.';
      } else {
        errorMessage = `Failed to initialize editor: ${err}`;
      }

      setError(errorMessage);
      setIsLoading(false);
    }
  };

  const loadEditor = async () => {
    try {
      const params = new URLSearchParams(window.location.search);
      if (params.get('library') === 'true') { setShowProjectLibrary(true); setIsLoading(false); return; }
      const projectId = params.get('project');
      if (projectId) {
        const document = parseProjectDocument(await invoke('load_project', { projectId }));
        useEditorStore.setState(restoredProjectState(document, useEditorStore.getState()));
        setMediaPath(document.state.videoFilePath);
        void useEditorStore.getState().loadThumbnails(document.state.videoFilePath!);
        void useEditorStore.getState().loadPreviewZoomData(document.state.videoFilePath!);
        setIsLoading(false);
        return;
      }

      const media = params.get('media');
      const sidecar = params.get('sidecar');
      const hasWebcam = params.get('webcam') === 'true';
      const hasMic = params.get('mic') === 'true';
      const hasSystemAudio = params.get('system_audio') === 'true';
      const webcamShape = normalizeWebcamShape(params.get('webcam_shape'));
      const webcamTransform = readWebcamTransform(params);
      const loading = params.get('loading') === 'true';
      const tempPath = params.get('temp_path');

      if (loading && tempPath) {
        setMediaPath(decodeURIComponent(tempPath));
        setProcessingStatus('Processing recording...');
        setIsLoading(true);

        setupProcessingEventListeners();
        return;
      }

      if (!media) {
        throw new Error('No media file specified in URL params');
      }

      const decodedPath = decodeURIComponent(media);
      setMediaPath(decodedPath);

      const videoInfo = await invoke<any>('get_video_metadata', { filePath: decodedPath });

      let duration = 10000;
      if (videoInfo.duration_ms) {
        duration = videoInfo.duration_ms;
      } else if (videoInfo.duration) {
        duration = videoInfo.duration * 1000;
      } else if (videoInfo.format && videoInfo.format.duration) {
        duration = parseFloat(videoInfo.format.duration) * 1000;
      }

      await initializeEditor(decodedPath, duration, hasWebcam, hasMic, hasSystemAudio, videoInfo.width ?? null, videoInfo.height ?? null);
      if (hasWebcam) {
        updateVisualSettings({ webcamShape, ...webcamTransform });
      }

      try {await applyDefaultPreset();setPresetWarning(null);}
      catch {setPresetWarning('The default style could not be loaded. Your recording is ready; open Style presets to reload the library.');}
      setIsLoading(false);
    } catch (err) {
      console.error('Failed to load editor:', err);
      setError(projectLoadError(err));
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadEditor();
  }, [initializeEditor, updateVisualSettings]);

  const handleClose = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('show_capture_bar');
    } catch (error) {
      console.error('Failed to show capture bar on close:', error);
    }

    if (window.history.length > 1) {
      window.history.back();
    } else {
      window.close();
    }
  };

  if (showProjectLibrary) return <ProjectLibrary onCancel={() => void handleClose()} />;

  if (error) {
    return (
      <div className="editor-container">
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100vh',
          gap: '16px',
          color: 'var(--editor-text-primary)',
          background: 'var(--editor-bg-primary)'
        }}>
          <div style={{ fontSize: '24px', fontWeight: 600 }}>Error Loading Editor</div>
          <div style={{ fontSize: '14px', color: 'var(--editor-text-secondary)', textAlign: 'center', maxWidth: '400px' }}>
            {error}
          </div>
          {error.includes('Recording is missing:') && new URLSearchParams(window.location.search).get('project') &&
            <RelinkRecording projectId={new URLSearchParams(window.location.search).get('project')!}
              onRelinked={()=>{setError(null);setIsLoading(true);void loadEditor();}} />}
          <div style={{ display: 'flex', gap: '12px', marginTop: '16px' }}>
            <button
              className="editor-btn editor-btn--secondary"
              onClick={() => {
                setError(null);
                setIsLoading(true);
                loadEditor();
              }}
            >
              Try Again
            </button>
            <button
              className="editor-btn editor-btn--primary"
              onClick={handleClose}
            >
              Go Back
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (isLoading || !mediaPath) {
    return (
      <div className="editor-container">
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100vh',
          gap: '16px',
          color: 'var(--editor-text-primary)',
          background: 'var(--editor-bg-primary)'
        }}>
          <div className="editor-spinner" />
          <div style={{ fontSize: '16px', fontWeight: 500 }}>{processingStatus}</div>
          <div style={{ fontSize: '13px', color: 'var(--editor-text-secondary)' }}>
            {processingStatus.includes('Processing') || processingStatus.includes('Finalizing') || processingStatus.includes('Applying')
              ? 'Your video is being processed in the background...'
              : 'Preparing your video for editing'
            }
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="editor-container">
      {presetWarning && <div className="editor-export-status" data-error><span role="alert">{presetWarning}</span>
        <button type="button" onClick={()=>setPresetWarning(null)}>Dismiss</button></div>}
      <ErrorBoundary>
        <EditorView onClose={handleClose} />
      </ErrorBoundary>
    </div>
  );
}

const root = document.getElementById('root');
if (root) {
  try {
    createRoot(root).render(
      <ErrorBoundary>
        <EditorShell />
      </ErrorBoundary>
    );
  } catch (err) {
    console.error('Failed to mount React app:', err);
    root.innerHTML = `
      <div style="padding: 20px; background: #1C1B19; color: #FCE8C3; height: 100vh;">
        <h1>Failed to mount React app</h1>
        <p>Error: ${err}</p>
      </div>
    `;
  }
} else {
  console.error('Root element not found!');
}
