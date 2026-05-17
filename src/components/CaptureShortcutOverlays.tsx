import React from 'react';

interface CaptureShortcutOverlaysProps {
  showShortcuts: boolean;
  showSpeakerNotes: boolean;
  onCloseShortcuts: () => void;
  onCloseSpeakerNotes: () => void;
}

const CaptureShortcutOverlays: React.FC<CaptureShortcutOverlaysProps> = ({
  showShortcuts,
  showSpeakerNotes,
  onCloseShortcuts,
  onCloseSpeakerNotes,
}) => (
  <>
    {showShortcuts && (
      <div className="shortcut-backdrop" onClick={onCloseShortcuts}>
        <div className="shortcut-modal" onClick={(event) => event.stopPropagation()}>
          <header>
            <h2>Keyboard Shortcuts</h2>
            <button onClick={onCloseShortcuts}>Esc</button>
          </header>
          <div className="shortcut-list">
            <div className="shortcut-row"><span>Start / finish recording</span><kbd>⌘ ⌥ ⌃ R</kbd></div>
            <div className="shortcut-row"><span>Restart recording</span><kbd>⌘ ⌥ ⌃ ⇧ R</kbd></div>
            <div className="shortcut-row"><span>Show speaker notes</span><kbd>⌘ ⌥ /</kbd></div>
            <div className="shortcut-row"><span>Show shortcuts</span><kbd>⌘ /</kbd></div>
            <div className="shortcut-row"><span>Record entire display</span><kbd>Customizable</kbd></div>
            <div className="shortcut-row"><span>Record single window</span><kbd>Customizable</kbd></div>
            <div className="shortcut-row"><span>Record area</span><kbd>Customizable</kbd></div>
          </div>
        </div>
      </div>
    )}

    {showSpeakerNotes && (
      <div className="speaker-notes-panel">
        <header>
          <span>Speaker Notes</span>
          <button onClick={onCloseSpeakerNotes}>×</button>
        </header>
        <textarea placeholder="Private notes for this recording..." />
      </div>
    )}
  </>
);

export default CaptureShortcutOverlays;
