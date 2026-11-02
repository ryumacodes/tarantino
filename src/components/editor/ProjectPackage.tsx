import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Channel, invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../stores/editor';
import { createProjectDocument } from '../../utils/projects/document';
import './project-package.css';

export default function ProjectPackage({ mode, onClose }: { mode: 'export' | 'import'; onClose: () => void }) {
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [progress, setProgress] = useState<{file_name:string;file_bytes:number;file_total:number} | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const operation = useRef<string | null>(null);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const pending = useRef(false);
  const alive = useRef(true);
  const panel = useRef<HTMLElement>(null);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    alive.current = true;
    const previous = document.activeElement as HTMLElement | null;
    input.current?.focus();
    return () => {
      alive.current = false;
      if (operation.current) void invoke('cancel_project_package', {operationId:operation.current}).catch(() => {});
      previous?.focus();
    };
  }, []);
  // Disabling the submit control during copying can move focus to the document body.
  useEffect(() => { if (!busy && !choosing) input.current?.focus(); }, [busy, choosing]);
  const choose = async () => {
    if (pending.current) return;
    pending.current = true; setChoosing(true); setError('');
    try {
      const selected = await invoke<string | null>('choose_project_package', { mode });
      if (alive.current && selected) { setPath(selected); setNotice(''); }
    } catch (error) { if (alive.current) setError(String(error)); }
    finally { pending.current = false; if (alive.current) setChoosing(false); }
  };
  const run = async () => {
    if (pending.current || !path.trim()) return;
    pending.current = true; setBusy(true); setError(''); setNotice(''); setProgress(null); setCancelling(false);
    const operationId = crypto.randomUUID(); operation.current = operationId;
    const channel = new Channel<{file_name:string;file_bytes:number;file_total:number}>();
    channel.onmessage = value => {
      if (operation.current !== operationId) return;
      if (!alive.current) { void invoke('cancel_project_package', {operationId}).catch(() => {}); return; }
      setProgress(value);
    };
    try {
      if (mode === 'export') {
        const state = useEditorStore.getState();
        if (state.zoomLoading) throw new Error('Wait for the recording to finish loading.');
        await invoke('export_project_package', { project: createProjectDocument(state), destination: path.trim(), operationId, progress: channel });
        if (alive.current) setNotice('Package saved. You can move the whole folder and import it on another computer.');
      } else {
        const id = await invoke<string>('import_project_package', { packagePath: path.trim(), operationId, progress: channel });
        if (alive.current) window.location.search = `?project=${encodeURIComponent(id)}`;
      }
    } catch (error) { if (alive.current) setError(String(error)); }
    finally { operation.current = null; pending.current = false; if (alive.current) { setBusy(false); setProgress(null); setCancelling(false); } }
  };
  const cancel = async () => {
    if (!operation.current || cancelling) return;
    setCancelling(true);
    try { await invoke('cancel_project_package', {operationId:operation.current}); }
    catch (error) { if (alive.current) { setError(String(error)); setCancelling(false); } }
  };
  return createPortal(<div className="project-package-backdrop" onKeyDown={event => {
    event.stopPropagation();
    if (event.key === 'Escape' && !pending.current) onClose();
    if (event.key !== 'Tab') return;
    const elements = Array.from(panel.current?.querySelectorAll<HTMLElement>('input:not(:disabled),button:not(:disabled)') ?? []);
    const first = elements[0], last = elements[elements.length - 1];
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }}>
    <section ref={panel} className="project-package-panel" role="dialog" aria-modal="true" aria-labelledby="package-heading">
      <h2 id="package-heading">{mode === 'export' ? 'Export project package' : 'Import project package'}</h2>
      <p>{mode === 'export' ? 'Save your edits, recordings, camera, audio and background assets together. Choose a new folder path; existing folders are never overwritten.' : 'Enter the path to a project package folder. Its media will be copied into your local library, so the original package can be moved or removed afterward.'}</p>
      <label htmlFor="package-path">{mode === 'export' ? 'New package folder' : 'Package folder'}</label>
      <input id="package-path" ref={input} value={path} disabled={busy || choosing} placeholder="/path/to/My project.tarantino-project"
        onChange={event => { setPath(event.target.value); setNotice(''); }} />
      <button className="editor-btn editor-btn--ghost" disabled={busy || choosing} onClick={() => void choose()}>
        {choosing ? 'Choosing…' : mode === 'export' ? 'Choose location…' : 'Choose folder…'}
      </button>
      {error && <p role="alert" className="project-package-error">{error}</p>}
      {(notice || busy) && <p role="status">{busy ? cancelling ? 'Cancelling and removing the incomplete copy…' : 'Copying project and media…' : notice}</p>}
      {busy && progress && <div className="project-package-progress">
        <p>{progress.file_name} · {(progress.file_bytes / 1048576).toFixed(1)} / {(progress.file_total / 1048576).toFixed(1)} MB</p>
        <progress aria-label="Current file copy progress" value={progress.file_bytes} max={Math.max(1, progress.file_total)} />
      </div>}
      <div className="project-package-actions">
        {busy && <button className="editor-btn editor-btn--ghost" disabled={!progress || cancelling} onClick={() => void cancel()}>Cancel copy</button>}
        <button className="editor-btn editor-btn--ghost" disabled={busy || choosing} onClick={onClose}>Close</button>
        <button className="editor-btn editor-btn--primary" disabled={busy || choosing || !path.trim() || !!notice} onClick={() => void run()}>
          {mode === 'export' ? 'Export package' : 'Import and open'}
        </button>
      </div>
    </section>
  </div>, document.body);
}
