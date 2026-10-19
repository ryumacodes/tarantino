import React, { useRef } from 'react';
import { useLinuxCaptureWindow } from './hooks/useLinuxCaptureWindow';
import CaptureBar from './components/CaptureBar';
import './styles/globals.css';
import './styles/capture-bar.css';
import './styles/webcam-overlay.css';
import './styles/capture-controls.css';
import './styles/editor-legacy.css';
import './styles/shortcuts.css';

// App.tsx is the entry point for the capture-bar window (index.html)
// It should ONLY render the CaptureBar component.
// EditorView is rendered in a dedicated editor window (editor.html → editor.tsx)
function App() {
  const captureWindow = useRef<HTMLDivElement>(null);
  useLinuxCaptureWindow(captureWindow);
  return (
    <div className="app" ref={captureWindow}>
      <CaptureBar />
    </div>
  );
}

export default App;
