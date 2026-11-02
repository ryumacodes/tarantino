import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {useEditorStore} from '../../../stores/editor';
import {type TranscriptWord} from '../../../utils/captions/document';
import {applyGeneratedTranscript} from '../../../stores/editor/generatedTranscript';
import {speechLocale,speechLanguageName,speechLanguages} from '../../../utils/captions/languages';
import type {TabProps} from './types';
interface Source {path:string;audioTrack:number;label:string}
interface Status {available:boolean;supportedLocales:string[];modelStatus?:string}

export default function CaptionGeneration({isExporting=false}:TabProps) {
  const revision=useEditorStore(state=>state.recordingRevision);
  return <Generation key={revision} disabled={isExporting}/>;
}
function savedLocale(path:string|null){return speechLocale(useEditorStore.getState().transcripts.find(session=>session.sourceFilePath===path)?.generator?.locale??'en-US');}
function Generation({disabled}:{disabled:boolean}) {
  const primary=useEditorStore(state=>state.videoFilePath);
  const clips=useEditorStore(state=>state.clips);
  const recordings=[...new Set([primary,...clips.filter(clip=>clip.type==='video'||clip.type==='screen').map(clip=>clip.sourceFilePath)].filter((path):path is string=>!!path))];
  const [recording,setRecording]=useState(primary??'');
  const [sources,setSources]=useState<Source[]>([]),[sourceIndex,setSourceIndex]=useState(0);
  const [locale,setLocale]=useState(()=>savedLocale(primary)),[status,setStatus]=useState<Status|null>(null);
  const [message,setMessage]=useState(''),[busy,setBusy]=useState(false);
  const [checking,setChecking]=useState(false),[loading,setLoading]=useState(false);
  const [sourceError,setSourceError]=useState(''),[statusError,setStatusError]=useState('');
  const [sourceAttempt,retrySources]=useState(0),[statusAttempt,retryStatus]=useState(0);
  const cancelled=useRef(false);
  const job=useRef<string|null>(null),alive=useRef(true),exporting=useRef(disabled);
  exporting.current=disabled;
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;if(job.current)void invoke('cancel_transcription',{jobId:job.current}).catch(()=>{});};},[]);
  useEffect(()=>{
    let current=true;setSources([]);setSourceIndex(0);setLoading(true);setSourceError('');
    if(!recording){setLoading(false);return;}
    void invoke<Source[]>('speech_sources',{sourceFilePath:recording}).then(value=>{if(current)setSources(value);})
      .catch(error=>{if(current)setSourceError(String(error));}).finally(()=>{if(current)setLoading(false);});
    return()=>{current=false;};
  },[recording,sourceAttempt]);
  useEffect(()=>{
    let current=true;setStatus(null);setChecking(true);setStatusError('');
    void invoke<Status>('speech_status',{locale}).then(value=>{if(current)setStatus(value);})
      .catch(error=>{if(current)setStatusError(String(error));}).finally(()=>{if(current)setChecking(false);});
    return()=>{current=false;};
  },[locale,statusAttempt]);
  async function run(install:boolean) {
    const source=sources[sourceIndex];if(job.current||disabled||(!install&&!source))return;
    const jobId=crypto.randomUUID();job.current=jobId;cancelled.current=false;setBusy(true);setMessage(install?'Preparing local speech model…':'Generating captions…');
    const snapshot=useEditorStore.getState();
    try {
      if(install) {
        await invoke('install_speech_model',{jobId,locale});
        const next=await invoke<Status>('speech_status',{locale});
        if(alive.current&&!cancelled.current){setStatus(next);setMessage(next.modelStatus==='installed'?'Speech model ready.':'Language setup has not completed. Try setup again.');}
      } else {
        const result=await invoke<{words:TranscriptWord[];locale:string}>('generate_transcript',{jobId,sourceFilePath:source.path,audioTrack:source.audioTrack,locale});
        if(!alive.current||cancelled.current)return;
        if(exporting.current){setMessage('Captions were not applied because an export is running. Generate again after it finishes.');return;}
        // Sidecar audio shares the recording clock, as in preview/export. Keep raw source timing.
        if(!applyGeneratedTranscript(snapshot,recording,result.words,{type:'system',locale:result.locale})){
          setMessage('The recording changed during generation. Generate again for the current recording.');return;
        }
        setMessage(result.words.length?`Generated ${result.words.length} words.`:'No speech was found in this audio.');
      }
    } catch(error) {if(alive.current)setMessage(String(error));}
    finally {job.current=null;if(alive.current){setBusy(false);if(cancelled.current)setMessage('Cancelled.');}}
  }
  const available=status?.available,ready=status?.modelStatus==='installed';
  const locales=speechLanguages(locale,status?.supportedLocales??[]);
  return <fieldset disabled={disabled}>
    <legend className="caption-heading">Generate captions</legend>
    <p className="setting-hint">Recognize speech locally. Model setup may download language files. Generating again replaces this recording’s transcript; Undo restores it.</p>
    <label className="caption-field">Recording<select value={recording} disabled={busy} onChange={event=>{setMessage('');setRecording(event.target.value);setLocale(savedLocale(event.target.value));}}>
      {recordings.map(path=><option key={path} value={path}>{path.split(/[\\/]/).pop()}</option>)}
    </select></label>
    <label className="caption-field">Audio<select value={sourceIndex} disabled={busy||loading} onChange={event=>setSourceIndex(Number(event.target.value))}>
      {sources.map((source,index)=><option key={index} value={index}>{source.label}</option>)}
    </select></label>
    {sourceError&&<div role="alert"><p className="setting-hint">Could not check this recording’s audio. {sourceError}</p><button type="button" disabled={busy||loading} onClick={()=>retrySources(value=>value+1)}>Retry audio check</button></div>}
    {!loading&&!sourceError&&!sources.length&&<p className="setting-hint">This recording has no available audio.</p>}
    <label className="caption-field">Language<select value={locale} disabled={busy||checking} onChange={event=>{setMessage('');setLocale(event.target.value);}}>
      {locales.map(value=><option key={value} value={value}>{speechLanguageName(value)}</option>)}
    </select></label>
    {statusError&&<div role="alert"><p className="setting-hint">Could not check local speech support. {statusError}</p><button type="button" disabled={busy||checking} onClick={()=>retryStatus(value=>value+1)}>Retry speech check</button></div>}
    <div className="caption-presets">
      {!ready&&<button type="button" disabled={busy||checking||!available} onClick={()=>void run(true)}>Set up language</button>}
      <button type="button" disabled={busy||!ready||!sources.length||loading} onClick={()=>void run(false)}>Generate</button>
      {busy&&<button type="button" onClick={()=>{cancelled.current=true;setMessage('Cancelling…');if(job.current)void invoke('cancel_transcription',{jobId:job.current}).catch(error=>setMessage(String(error)));}}>Cancel</button>}
    </div>
    <p className="setting-hint" role="status">{message||(checking?'Checking local speech support…':status&&!available?'Local speech recognition is unavailable on this device.':'')}</p>
  </fieldset>;
}
