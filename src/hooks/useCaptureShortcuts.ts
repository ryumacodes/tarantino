import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';

interface UseCaptureShortcutsOptions {
  isRecording: boolean;
  isStarting: boolean;
  onChooseDisplay: () => void | Promise<void>;
  onChooseWindow: () => void | Promise<void>;
  onShowCaptureControls: () => void;
  onOpenSettings: () => void;
  onOpenShortcuts: () => void;
  showShortcuts: boolean;
  showSpeakerNotes: boolean;
  onToggleShortcuts: () => void;
  onCloseShortcuts: () => void;
  onToggleSpeakerNotes: () => void;
  onCloseSpeakerNotes: () => void;
  onFinishRecording: () => void;
  onStartRecording: () => void;
  onRestartRecording: () => void;
}

export const useCaptureShortcuts = ({
  isRecording,
  isStarting, onChooseDisplay, onChooseWindow, onShowCaptureControls, onOpenSettings, onOpenShortcuts,
  showShortcuts,
  showSpeakerNotes,
  onToggleShortcuts,
  onCloseShortcuts,
  onToggleSpeakerNotes,
  onCloseSpeakerNotes,
  onFinishRecording,
  onStartRecording,
  onRestartRecording,
}: UseCaptureShortcutsOptions) => {
  const menuActions = useRef({ isRecording, isStarting, onChooseDisplay, onChooseWindow, onShowCaptureControls, onOpenSettings, onOpenShortcuts });
  menuActions.current = { isRecording, isStarting, onChooseDisplay, onChooseWindow, onShowCaptureControls, onOpenSettings, onOpenShortcuts };
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    listen<string>('capture-menu-action', async ({ payload }) => {
      if (disposed) return;
      const actions = menuActions.current;
      if (actions.isRecording || actions.isStarting) return;
      try {
        if (payload === 'new') actions.onShowCaptureControls();
        else if (payload === 'display') await actions.onChooseDisplay();
        else if (payload === 'window') await actions.onChooseWindow();
        else if (payload === 'settings') actions.onOpenSettings();
        else if (payload === 'shortcuts') actions.onOpenShortcuts();
      } catch (error) {
        console.error('Capture menu action failed:', error);
        window.alert(`Could not open capture controls: ${String(error)}`);
      }
    }).then(unlisten => { if (disposed) unlisten(); else cleanup = unlisten; })
      .catch(error => console.error('Could not listen for capture menu actions:', error));
    return () => { disposed = true; cleanup?.(); };
  }, []);
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      const key = event.key.toLowerCase();
      if (event.key === 'Escape') {
        if (showShortcuts) onCloseShortcuts();
        else if (showSpeakerNotes) onCloseSpeakerNotes();
        return;
      }
      if (event.metaKey && !event.altKey && !event.ctrlKey && key === '/') {
        event.preventDefault();
        onToggleShortcuts();
      } else if (event.metaKey && event.altKey && !event.ctrlKey && key === '/') {
        event.preventDefault();
        onToggleSpeakerNotes();
      } else if (event.metaKey && event.altKey && event.ctrlKey && event.shiftKey && key === 'r') {
        event.preventDefault();
        onRestartRecording();
      } else if (event.metaKey && event.altKey && event.ctrlKey && key === 'r') {
        event.preventDefault();
        if (isRecording) onFinishRecording();
        else onStartRecording();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [
    isRecording,
    showShortcuts,
    showSpeakerNotes,
    onToggleShortcuts,
    onCloseShortcuts,
    onToggleSpeakerNotes,
    onCloseSpeakerNotes,
    onFinishRecording,
    onStartRecording,
    onRestartRecording,
  ]);
};
