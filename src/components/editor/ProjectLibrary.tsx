import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { ArrowUpRight, Film, FolderOpen, Search, X } from 'lucide-react';
import ProjectPreview from './ProjectPreview';
import ProjectPackage from './ProjectPackage';
import './project-library.css';
interface ProjectSummary { id: string; title: string; modified_ms: number; media_path?: string; duration_ms?: number }
interface RecordingSummary { path: string; title: string; modified_ms: number; has_microphone: boolean; has_system_audio: boolean; has_webcam: boolean }
function title(value: string) {
  const stamp = /^(?:recording_|Recording )(\d{4})(\d{2})(\d{2})_(\d{2})(\d{2})(\d{2})$/.exec(value);
  return stamp ? `Recording · ${stamp[4]}:${stamp[5]}` : value;
}
function date(ms: number) { return new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }); }
function duration(ms?: number) { if (!ms) return ''; const s = Math.round(ms / 1000); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; }
export default function ProjectLibrary({ onCancel }: { onCancel: () => void }) {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [recordings, setRecordings] = useState<RecordingSummary[]>([]);
  const [error, setError] = useState('');
  const [recoveryError, setRecoveryError] = useState('');
  const [loading, setLoading] = useState(true);
  const [recordingsLoading, setRecordingsLoading] = useState(true);
  const [tab, setTab] = useState<'projects' | 'recordings'>('projects');
  const [query, setQuery] = useState('');
  const [showImport, setShowImport] = useState(false);
  const panel = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  useEffect(() => {
    let disposed = false;
    const previous = document.activeElement as HTMLElement | null;
    search.current?.focus();
    invoke<ProjectSummary[]>('list_saved_projects').then(items => { if (!disposed) setProjects(items); })
      .catch(e => { if (!disposed) setError(`Could not load projects: ${String(e)}`); }).finally(() => { if (!disposed) setLoading(false); });
    invoke<RecordingSummary[]>('list_unsaved_recordings').then(items => { if (!disposed) setRecordings(items); })
      .catch(e => { if (!disposed) setRecoveryError(`Could not load recordings: ${String(e)}`); }).finally(() => { if (!disposed) setRecordingsLoading(false); });
    return () => { disposed = true; previous?.focus(); };
  }, []);
  const needle = query.trim().toLocaleLowerCase();
  const items = tab === 'projects'
    ? projects.map(p => ({ key: p.id, title: p.title, modified: p.modified_ms, path: p.media_path, duration: p.duration_ms,
      open: () => { window.location.search = `?project=${encodeURIComponent(p.id)}`; } }))
    : recordings.map(r => ({ key: r.path, title: r.title, modified: r.modified_ms, path: r.path, duration: undefined,
      open: () => { window.location.search = new URLSearchParams({ media: r.path, mic: String(r.has_microphone), system_audio: String(r.has_system_audio), webcam: String(r.has_webcam) }).toString(); } }));
  const filtered = items.filter(item => `${item.title} ${title(item.title)} ${date(item.modified)}`.toLocaleLowerCase().includes(needle));
  const busy = tab === 'projects' ? loading : recordingsLoading;
  const problem = tab === 'projects' ? error : recoveryError;
  return createPortal(<div className="project-library-backdrop" role="dialog" aria-modal="true" aria-labelledby="project-library-title"
    onKeyDown={event => {
      if (event.key === 'Escape') { event.stopPropagation(); onCancel(); }
      if (event.key !== 'Tab') return;
      const focusable = Array.from(panel.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input') ?? []);
      const first = focusable[0], last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }}>
    <div className="project-library" ref={panel}>
      <header className="project-library-header">
        <div className="project-library-heading"><span className="project-library-brand"><Film size={15} /> TARANTINO</span>
          <h1 id="project-library-title">Your projects<span>.</span></h1><p>Pick up where you left off.</p></div>
        <button className="project-library-close" aria-label="Close project library" onClick={onCancel}><X size={20} /></button>
      </header>
      <div className="project-library-toolbar">
        <button className="editor-btn editor-btn--ghost" onClick={() => setShowImport(true)}>Import package</button>
        <div className="project-library-tabs" aria-label="Library sections">
          <button aria-pressed={tab === 'projects'} onClick={() => setTab('projects')}><FolderOpen size={16} /> Projects <span>{projects.length}</span></button>
          <button aria-pressed={tab === 'recordings'} onClick={() => setTab('recordings')}><Film size={16} /> Recordings <span>{recordings.length}</span></button>
        </div>
        <label className="project-library-search"><Search size={17} /><input ref={search} value={query} onChange={e => setQuery(e.target.value)} placeholder="Search your library" aria-label="Search your library" />
          {query && <button aria-label="Clear search" onClick={() => { setQuery(''); search.current?.focus(); }}><X size={14} /></button>}</label>
      </div>
      <div className="project-library-content">
        <div className="project-library-caption"><span>{tab === 'projects' ? 'RECENT PROJECTS' : 'ORIGINAL RECORDINGS'}</span><span>Newest first</span></div>
        {tab === 'recordings' && <p className="project-library-hint">Recordings that haven’t been saved as projects. Open one to continue editing.</p>}
        {problem ? <p role="alert" className="project-library-empty">{problem}</p> : busy ? <p role="status" className="project-library-empty">Loading your library…</p> : filtered.length ?
          <div className="project-library-grid">{filtered.map(item => <button className="project-card" key={item.key} onClick={item.open} aria-label={`Open ${title(item.title)}`} title={item.title}>
            <div className="project-card-media"><ProjectPreview path={item.path} /><span className="project-card-open"><ArrowUpRight size={21} /></span>{duration(item.duration) && <span className="project-card-duration">{duration(item.duration)}</span>}</div>
            <div className="project-card-info"><h2>{title(item.title)}</h2><p>{date(item.modified)}<span>{tab === 'projects' ? 'Project' : 'Recording'}</span></p></div>
          </button>)}</div> : <div className="project-library-empty"><FolderOpen size={32} strokeWidth={1.3} /><h2>{query ? 'No matches' : tab === 'projects' ? 'Your next story starts here' : 'All caught up'}</h2><p>{query ? 'Try another name or date.' : tab === 'projects' ? 'Saved projects will appear here. You can also open an existing recording.' : 'There are no recordings waiting to be opened.'}</p></div>}
      </div>
      <footer className="project-library-footer"><span>{filtered.length} {tab === 'projects' ? (filtered.length === 1 ? 'project' : 'projects') : (filtered.length === 1 ? 'recording' : 'recordings')}</span><span>Stored locally on your Mac</span><span><kbd>esc</kbd> Close</span></footer>
    </div>
    {showImport && <ProjectPackage mode="import" onClose={() => setShowImport(false)} />}
  </div>, document.body);
}
