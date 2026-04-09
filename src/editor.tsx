import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import EditorView from './components/EditorView';
import { useEditorStore } from './stores/editor';
import './styles/globals.css';
import './styles/dracula-theme.css';

console.log('%c[EDITOR] Module loaded!', 'background: blue; color: white; font-size: 20px;');

// Add error boundary for debugging
class ErrorBoundary extends React.Component<{ children: React.ReactNode }, { hasError: boolean, error?: Error }> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error) {
    console.error('Error caught by boundary:', error);
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: any) {
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

// Set up event listeners for background processing updates
let setupProcessingEventListeners: () => void;

function EditorShell() {
  const [mediaPath, setMediaPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [processingStatus, setProcessingStatus] = useState<string>('Loading...');

  // Helper for dual logging to console and terminal
  const log = (message: string, level: 'info' | 'warn' | 'error' = 'info') => {
    // Log to browser console
    if (level === 'error') console.error(message);
    else if (level === 'warn') console.warn(message);
    else console.log(message);

    // Log to backend terminal
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('log_to_terminal', { message, level }).catch(() => { });
    });
  };

  // Use store directly without try-catch wrapping
  const { initializeEditor } = useEditorStore();

  // Set up event listeners for background processing updates
  setupProcessingEventListeners = () => {
    console.log('Setting up processing event listeners');

    // Listen for processing status updates
    listen('processing-status', (event: any) => {
      console.log('Processing status update:', event.payload);
      setProcessingStatus(event.payload);
    });

    // Listen for recording ready event
    listen('recording-ready', async (event: any) => {
      log(`Recording ready event received: ${JSON.stringify(event.payload)}`);
      const payload = event.payload;
      const finalPath = typeof payload === 'string' ? payload : payload.path;
      const hasMic = typeof payload === 'object' ? !!payload.has_mic : false;
      const hasSystemAudio = typeof payload === 'object' ? !!payload.has_system_audio : false;
      const hasWebcamFromPayload = typeof payload === 'object' ? !!payload.has_webcam : false;
      // Also check URL params (set when editor opened)
      const urlHasWebcam = new URLSearchParams(window.location.search).get('webcam') === 'true';
      const webcamEnabled = hasWebcamFromPayload || urlHasWebcam;

      // Update state to trigger React re-render and proper video initialization
      setMediaPath(finalPath);
      setProcessingStatus('Ready!');
      setIsLoading(true); // Reset loading state to trigger re-initialization

      // Initialize the editor with the final path
      initializeEditorWithPath(finalPath, hasMic, hasSystemAudio, webcamEnabled);
    });
  };

  const initializeEditorWithPath = async (path: string, hasMic = false, hasSystemAudio = false, hasWebcam = false) => {
    try {
      console.log('Initializing editor with final path:', path, 'hasWebcam:', hasWebcam);

      // Get actual video duration and metadata
      const videoInfo = await invoke<any>('get_video_metadata', { filePath: path });
      log(`Video metadata loaded: ${JSON.stringify(videoInfo)}`);

      // Parse duration from different possible formats
      let duration = 10000; // Fallback to 10 seconds
      if (videoInfo.duration_ms) {
        duration = videoInfo.duration_ms;
      } else if (videoInfo.duration) {
        duration = videoInfo.duration * 1000; // Convert seconds to milliseconds
      }

      console.log('Initializing editor with duration:', duration, 'video size:', videoInfo.width, 'x', videoInfo.height);
      await initializeEditor(path, duration, hasWebcam, hasMic, hasSystemAudio, videoInfo.width ?? null, videoInfo.height ?? null);

      setIsLoading(false);
      log('Editor initialization complete');
    } catch (err) {
      log(`Error initializing editor with final path: ${err}`, 'error');

      // Provide more specific error messages based on the error type
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
      console.log('Editor component mounted');
      console.log('Current URL:', window.location.href);
      console.log('Search params:', window.location.search);

      const params = new URLSearchParams(window.location.search);
      const media = params.get('media');
      const sidecar = params.get('sidecar');
      const hasWebcam = params.get('webcam') === 'true';
      const hasMic = params.get('mic') === 'true';
      const hasSystemAudio = params.get('system_audio') === 'true';
      const loading = params.get('loading') === 'true';
      const tempPath = params.get('temp_path');

      console.log('Editor loaded with params:', { media, sidecar, hasWebcam, hasMic, hasSystemAudio, loading, tempPath });

      // Handle loading mode (instant editor opening)
      if (loading && tempPath) {
        console.log('Editor opened in loading mode with temp path:', tempPath);
        setMediaPath(decodeURIComponent(tempPath));
        setProcessingStatus('Processing recording...');
        setIsLoading(true);

        // Set up event listeners for background processing updates
        setupProcessingEventListeners();
        return;
      }

      if (!media) {
        throw new Error('No media file specified in URL params');
      }

      const decodedPath = decodeURIComponent(media);
      console.log('Media path decoded:', decodedPath);
      setMediaPath(decodedPath);

      // Get actual video duration and metadata
      console.log('Loading video metadata for:', decodedPath);
      const videoInfo = await invoke<any>('get_video_metadata', { filePath: decodedPath });
      console.log('Video metadata loaded:', videoInfo);

      // Parse duration from different possible formats
      let duration = 10000; // Fallback to 10 seconds
      if (videoInfo.duration_ms) {
        duration = videoInfo.duration_ms;
      } else if (videoInfo.duration) {
        // If duration is in seconds, convert to milliseconds
        duration = videoInfo.duration * 1000;
      } else if (videoInfo.format && videoInfo.format.duration) {
        // FFprobe format duration in seconds
        duration = parseFloat(videoInfo.format.duration) * 1000;
      }

      console.log('Parsed video duration:', duration, 'ms', 'video size:', videoInfo.width, 'x', videoInfo.height);

      // Initialize the editor store with the video file and media flags
      console.log('Initializing editor with path:', decodedPath, 'duration:', duration);
      initializeEditor(decodedPath, duration, hasWebcam, hasMic, hasSystemAudio, videoInfo.width ?? null, videoInfo.height ?? null);

      // Verify initialization worked
      setTimeout(() => {
        const currentStore = useEditorStore.getState();
        console.log('Editor initialization verification:', {
          videoFilePath: currentStore.videoFilePath,
          duration: currentStore.duration,
          hasWebcam: currentStore.hasWebcam,
          hasMicrophone: currentStore.hasMicrophone
        });
      }, 100);

      setIsLoading(false);
    } catch (err) {
      console.error('Failed to load editor:', err);
      setError(err instanceof Error ? err.message : 'Failed to load editor');
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadEditor();
  }, [initializeEditor]);

  const handleClose = async () => {
    console.log('Editor closing');

    // Show the capture bar when closing the editor
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('show_capture_bar');
      console.log('Capture bar shown on editor close');
    } catch (error) {
      console.error('Failed to show capture bar on close:', error);
    }

    // Navigate back or close the window
    if (window.history.length > 1) {
      window.history.back();
    } else {
      window.close();
    }
  };

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
      <ErrorBoundary>
        <EditorView onClose={handleClose} />
      </ErrorBoundary>
    </div>
  );
}

console.log('Starting editor app...');

const root = document.getElementById('root');
if (root) {
  console.log('Root element found, mounting React app...');
  try {
    createRoot(root).render(
      <ErrorBoundary>
        <EditorShell />
      </ErrorBoundary>
    );
    console.log('React app mounted successfully');
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