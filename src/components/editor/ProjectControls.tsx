import { saveProject } from '../../utils/projects/save';
import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import ProjectLibrary from './ProjectLibrary';
import ProjectPackage from './ProjectPackage';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEditorStore } from '../../stores/editor';
import { createProjectDocument } from '../../utils/projects/document';

export default function ProjectControls({ disabled }: { disabled: boolean }) {
  const id = useRef<string | null>(new URLSearchParams(window.location.search).get('project'));
  const busy = useRef(false);
  const lastSaved = useRef('');
  const [status, setStatus] = useState('');
  const [showLibrary, setShowLibrary] = useState(false);
  const [showPackage, setShowPackage] = useState(false);
  const save = useCallback(async (manual = true) => {
    if (busy.current || disabled) return false;
    const state = useEditorStore.getState();
    if (!state.videoFilePath || state.zoomLoading) return false;
    const project = createProjectDocument(state);
    const serialized = JSON.stringify(project);
    if (!manual && serialized === lastSaved.current) return true;
    busy.current = true;
    try {
      const savedId = await saveProject(project, id.current);
      id.current = savedId;
      lastSaved.current = serialized;
      window.history.replaceState(null, '', `?project=${encodeURIComponent(savedId)}`);
      setStatus(manual ? 'Saved' : 'Autosaved');
      return true;
    } catch (error) {
      setStatus(`Save failed: ${String(error)}`);
      return false;
    } finally { busy.current = false; }
  }, [disabled]);
  useEffect(() => {
    const timer = setInterval(() => { void save(false).catch(error => setStatus(String(error))); }, 30_000);
    const keydown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault(); void save().catch(error => setStatus(String(error)));
      }
    };
    const nativeSave = () => { void save().catch(error => setStatus(String(error))); };
    document.addEventListener('keydown', keydown);
    window.addEventListener('tarantino-save', nativeSave);
    return () => {
      clearInterval(timer);
      document.removeEventListener('keydown', keydown);
      window.removeEventListener('tarantino-save', nativeSave);
    };
  }, [save]);
  useEffect(() => {
    let disposed = false;
    let allowClose = false;
    let cleanup: (() => void) | undefined;
    const window = getCurrentWindow();
    window.onCloseRequested(async event => {
      if (allowClose) return;
      event.preventDefault();
      try {
        if (await save(false)) {
          await invoke('show_capture_bar');
          allowClose = true;
          await window.close();
        } else setStatus('Please wait for loading or saving before closing.');
      } catch (error) { setStatus(`Could not save before closing: ${String(error)}`); }
    }).then(unlisten => { if (disposed) unlisten(); else cleanup = unlisten; })
      .catch(error => setStatus(`Could not enable save on close: ${String(error)}`));
    return () => { disposed = true; cleanup?.(); };
  }, [save]);
  const open = async () => {
    try {
      if (!await save(false)) { setStatus('Wait for the current recording to finish loading or saving.'); return; }
      setShowLibrary(true);
    } catch (error) { setStatus(`Could not open projects: ${String(error)}`); }
  };
  const openRef = useRef(open);
  openRef.current = open;
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    listen('project-library-open', () => { if (!disposed) void openRef.current(); })
      .then(unlisten => { if (disposed) unlisten(); else cleanup = unlisten; })
      .catch(error => setStatus(String(error)));
    return () => { disposed = true; cleanup?.(); };
  }, []);
  return <>
    <span role="status" title={status} style={{ maxWidth: 180, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontSize: 11 }}>{status}</span>
    <button className="editor-btn editor-btn--ghost" onClick={() => void open()} disabled={disabled}>Open</button>
    <button className="editor-btn editor-btn--ghost" onClick={() => void save().catch(error => setStatus(String(error)))} disabled={disabled}>Save</button>
    <button className="editor-btn editor-btn--ghost" disabled={disabled} onClick={() => setShowPackage(true)}>Package</button>
    {showPackage && <ProjectPackage mode="export" onClose={() => setShowPackage(false)} />}
    {showLibrary && <ProjectLibrary onCancel={() => setShowLibrary(false)} />}
  </>;
}
