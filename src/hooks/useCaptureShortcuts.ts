import { useEffect } from 'react';

interface UseCaptureShortcutsOptions {
  isRecording: boolean;
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
