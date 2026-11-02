import { useEffect, useState } from 'react';
import { useEditorStore } from '../../../stores/editor';
import type { ZoomAnalysis } from '../../../stores/editor/types';

type Notice = {
  text: string;
  analysis: ZoomAnalysis | null;
  revision: number;
};

export function RebuildZooms({ disabled }: { disabled: boolean }) {
  const path = useEditorStore(state => state.videoFilePath);
  const revision = useEditorStore(state => state.recordingRevision);
  const analysis = useEditorStore(state => state.zoomAnalysis);
  const regenerate = useEditorStore(state => state.regenerateZooms);
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  useEffect(() => { setNotice(null); setPending(false); }, [path, revision]);

  const rebuild = async () => {
    const isCurrent = () => {
      const state = useEditorStore.getState();
      return state.videoFilePath === path && state.recordingRevision === revision;
    };
    setPending(true);
    setNotice(null);
    try {
      const applied = await regenerate();
      if (applied && isCurrent()) {
        const result = useEditorStore.getState().zoomAnalysis;
        const automatic = result?.zoom_blocks.filter(block => !block.is_manual) ?? [];
        const typing = automatic.filter(block => block.kind === 'typing').length;
        const clicks = automatic.length - typing;
        const text = automatic.length
          ? `Rebuilt ${automatic.length} automatic zoom${automatic.length === 1 ? '' : 's'}: ${clicks} click/drag, ${typing} typing. Undo is available.`
          : 'No automatic zooms were generated from this recording’s input. You can add zooms manually or undo this rebuild.';
        setNotice({ text, analysis: result, revision });
      }
    } catch (error) {
      if (isCurrent()) setNotice({ text: String(error), analysis: useEditorStore.getState().zoomAnalysis, revision });
    } finally {
      if (isCurrent()) setPending(false);
    }
  };
  // Editing or undoing the result invalidates its summary immediately.
  const message = notice?.analysis === analysis && notice?.revision === revision ? notice.text : '';
  return <div className="section">
    <button className="editor-btn editor-btn--secondary" disabled={disabled || pending || !path} onClick={rebuild}>
      {pending ? 'Analyzing recorded input…' : 'Rebuild automatic zooms'}
    </button>
    <p className="setting-hint">Replace automatic zooms from recorded clicks, drags, and typing. Manual zooms keep their ranges.</p>
    {message && <p className="setting-hint" role="status">{message}</p>}
  </div>;
}
