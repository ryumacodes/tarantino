import { useHistoryGestures } from '../../hooks/useHistoryGestures';
import { useEffect } from 'react';
import { Undo2, Redo2 } from 'lucide-react';
import { useEditorStore } from '../../stores/editor';

export default function HistoryControls({ disabled = false }: { disabled?: boolean }) {
  useHistoryGestures(disabled);
  const canUndo = useEditorStore(state => state.canUndo);
  const canRedo = useEditorStore(state => state.canRedo);
  const undo = useEditorStore(state => state.undo);
  const redo = useEditorStore(state => state.redo);
  useEffect(() => {
    const onNativeHistory = (event: Event) => {
      const target = document.activeElement;
      // Let WebKit handle typing history when focus is in a form field.
      if (target instanceof HTMLElement && (target.isContentEditable || target.closest('input,textarea,select'))) return;
      const action = (event as CustomEvent).detail;
      if (action !== 'undo' && action !== 'redo') return;
      event.preventDefault();
      if (!disabled) { if (action === 'undo') undo(); else redo(); }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (disabled || event.defaultPrevented || event.altKey || (!event.metaKey && !event.ctrlKey)) return;
      if (target instanceof HTMLElement && (target.isContentEditable || target.closest('input,textarea,select'))) return;
      const key = event.key.toLowerCase();
      if (key === 'z') {
        event.preventDefault();
        if (event.shiftKey) redo(); else undo();
      } else if (event.ctrlKey && !event.metaKey && key === 'y') {
        event.preventDefault(); redo();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    window.addEventListener('tarantino-history', onNativeHistory);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('tarantino-history', onNativeHistory);
    };
  }, [disabled, undo, redo]);
  return <div className="editor-history-controls" role="group" aria-label="Edit history">
    <button type="button" className="editor-btn" aria-label="Undo" title="Undo (⌘/Ctrl Z)"
      disabled={disabled || !canUndo} onClick={undo}><Undo2 size={16}/></button>
    <button type="button" className="editor-btn" aria-label="Redo" title="Redo (⌘/Ctrl Shift Z)"
      disabled={disabled || !canRedo} onClick={redo}><Redo2 size={16}/></button>
  </div>;
}
