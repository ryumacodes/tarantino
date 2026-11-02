import {useMemo,useRef,useState} from 'react';
import {useEditorStore} from '../../../stores/editor';
import type {TranscriptWord} from '../../../utils/captions/document';
import type {TabProps} from './types';

const PAGE_SIZE=40;
function WordEditor({sessionId,word,offsetMs,disabled}:{sessionId:string;word:TranscriptWord;offsetMs:number;disabled:boolean}) {
  const [draft,setDraft]=useState(word.text);
  const cancelled=useRef(false);
  const seconds=Math.floor((word.startMs+offsetMs)/100)/10;
  const timestamp=`${Math.floor(seconds/60)}:${(seconds%60).toFixed(1).padStart(4,'0')}`;
  return <div className={`transcript-word ${word.isHidden?'transcript-word-hidden':''}`}>
    <label className="transcript-visibility"><input type="checkbox" checked={!word.isHidden} disabled={disabled}
      aria-label={`Show ${word.text||'empty word'} at ${timestamp}`} onChange={event=>
        useEditorStore.getState().setTranscriptWordHidden(sessionId,word.id,!event.target.checked)}/></label>
    <input className="transcript-text" aria-label={`Transcript text at ${timestamp}`} value={draft} maxLength={10000}
      disabled={disabled} onChange={event=>setDraft(event.target.value)}
      onBlur={()=>{
        if(!cancelled.current&&draft!==word.text)useEditorStore.getState().updateTranscriptText(sessionId,word.id,draft);
        cancelled.current=false;
      }} onKeyDown={event=>{
        if(event.key==='Enter'){event.preventDefault();event.currentTarget.blur();}
        if(event.key==='Escape'){event.preventDefault();cancelled.current=true;setDraft(word.text);event.currentTarget.blur();}
        event.stopPropagation();
      }}/>
    <span className="transcript-time" title={word.type==='sound-label'?'Sound description':'Source recording time'}>{timestamp}</span>
  </div>;
}

export default function TranscriptEditor({isExporting=false}:TabProps) {
  const sessions=useEditorStore(state=>state.transcripts);
  const [selected,setSelected]=useState(''),[query,setQuery]=useState(''),[page,setPage]=useState(0);
  const session=sessions.find(item=>item.id===selected)??sessions[0];
  const words=useMemo(()=>{
    const needle=query.trim().toLocaleLowerCase();
    return session?.words.filter(word=>!needle||word.text.toLocaleLowerCase().includes(needle))??[];
  },[session,query]);
  const lastPage=Math.max(0,Math.ceil(words.length/PAGE_SIZE)-1),current=Math.min(page,lastPage);
  if(!session)return null;
  return <details className="transcript-editor">
    <summary>Transcript · {session.words.length} entries</summary>
    <p className="setting-hint">Correct text, then press Enter or leave the field to save. Escape cancels. Uncheck an entry to hide it. Times refer to the source recording.</p>
    {sessions.length>1&&<label className="caption-field">Recording<select value={session.id}
      onChange={event=>{setSelected(event.target.value);setPage(0);}}>
      {sessions.map((item,index)=><option key={item.id} value={item.id}>
        {index+1}. {item.sourceFilePath.split(/[\\/]/).pop()||'Recording'}
      </option>)}
    </select></label>}
    <input type="search" aria-label="Search transcript" placeholder="Search transcript…" value={query}
      onChange={event=>{setQuery(event.target.value);setPage(0);}}/>
    {!words.length&&<p className="setting-hint">No matching entries.</p>}
    {words.slice(current*PAGE_SIZE,(current+1)*PAGE_SIZE).map(word=><WordEditor
      key={JSON.stringify([session.id,word.id,word.text])} sessionId={session.id} word={word} offsetMs={session.offsetMs} disabled={isExporting}/>)}
    {lastPage>0&&<div className="caption-presets transcript-pagination">
      <button type="button" disabled={current===0} onClick={()=>setPage(current-1)}>Previous</button>
      <span>{current+1} / {lastPage+1}</span>
      <button type="button" disabled={current===lastPage} onClick={()=>setPage(current+1)}>Next</button>
    </div>}
  </details>;
}
