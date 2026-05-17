import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/globals.css';
import './styles/capture-bar.css';
import './styles/webcam-overlay.css';
import './styles/capture-controls.css';
import './styles/editor-legacy.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);