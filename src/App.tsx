import React from 'react';
import CaptureBar from './components/CaptureBar';
import './styles/globals.css';

// App.tsx is the entry point for the capture-bar window (index.html)
// It should ONLY render the CaptureBar component.
// EditorView is rendered in a dedicated editor window (editor.html → editor.tsx)
function App() {
  return (
    <div className="app">
      <CaptureBar />
    </div>
  );
}

export default App;
