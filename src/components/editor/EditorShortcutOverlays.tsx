import React, { useMemo, useState } from 'react';

export type EditorShortcutAction =
  | 'export'
  | 'play-pause'
  | 'shortcuts'
  | 'notes'
  | 'flag'
  | 'close';

interface ShortcutItem {
  label: string;
  keys: string;
  action?: EditorShortcutAction;
}

interface EditorShortcutOverlaysProps {
  commandOpen: boolean;
  shortcutsOpen: boolean;
  notesOpen: boolean;
  flags: number[];
  onCloseCommand: () => void;
  onCloseShortcuts: () => void;
  onCloseNotes: () => void;
  onAction: (action: EditorShortcutAction) => void;
}

export const EDITOR_SHORTCUTS: ShortcutItem[] = [
  { label: 'Show shortcuts', keys: '⌘ /', action: 'shortcuts' },
  { label: 'Command menu', keys: '⌘ K' },
  { label: 'Show speaker notes', keys: '⌘ ⌥ /', action: 'notes' },
  { label: 'Create recording flag', keys: '⌘ ⌥ ⌃ F', action: 'flag' },
  { label: 'Start / finish recording', keys: '⌘ ⌥ ⌃ R' },
  { label: 'Restart recording', keys: '⌘ ⌥ ⌃ ⇧ R' },
  { label: 'Export video', keys: '⌘ E', action: 'export' },
  { label: 'Play or pause preview', keys: 'Space', action: 'play-pause' },
  { label: 'Selection tool', keys: 'V' },
  { label: 'Scissors tool', keys: 'C' },
  { label: 'Trim tool', keys: 'T' },
  { label: 'Delete selected clip or zoom', keys: 'Delete / Backspace' },
  { label: 'Close project', keys: 'Esc', action: 'close' },
];

const formatTime = (ms: number) => {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
};

export const EditorShortcutOverlays: React.FC<EditorShortcutOverlaysProps> = ({
  commandOpen,
  shortcutsOpen,
  notesOpen,
  flags,
  onCloseCommand,
  onCloseShortcuts,
  onCloseNotes,
  onAction,
}) => {
  const [query, setQuery] = useState('');
  const commands = useMemo(
    () => EDITOR_SHORTCUTS.filter((item) => item.action && item.label.toLowerCase().includes(query.toLowerCase())),
    [query]
  );

  return (
    <>
      {shortcutsOpen && (
        <div className="shortcut-backdrop" onClick={onCloseShortcuts}>
          <div className="shortcut-modal" onClick={(event) => event.stopPropagation()}>
            <header>
              <h2>Keyboard Shortcuts</h2>
              <button onClick={onCloseShortcuts}>Esc</button>
            </header>
            <div className="shortcut-list">
              {EDITOR_SHORTCUTS.map((item) => (
                <div className="shortcut-row" key={item.label}>
                  <span>{item.label}</span>
                  <kbd>{item.keys}</kbd>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {commandOpen && (
        <div className="shortcut-backdrop" onClick={onCloseCommand}>
          <div className="command-menu" onClick={(event) => event.stopPropagation()}>
            <input
              autoFocus
              placeholder="Search commands"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            <div className="command-list">
              {commands.map((item) => (
                <button key={item.label} onClick={() => item.action && onAction(item.action)}>
                  <span>{item.label}</span>
                  <kbd>{item.keys}</kbd>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {notesOpen && (
        <div className="speaker-notes-panel">
          <header>
            <span>Speaker Notes</span>
            <button onClick={onCloseNotes}>×</button>
          </header>
          <textarea placeholder="Private notes for your recording..." />
        </div>
      )}

      {flags.length > 0 && (
        <div className="recording-flags">
          {flags.slice(-3).map((flag) => (
            <span key={flag}>Flag {formatTime(flag)}</span>
          ))}
        </div>
      )}
    </>
  );
};
