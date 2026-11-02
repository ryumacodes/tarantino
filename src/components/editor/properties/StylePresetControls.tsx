import { invoke } from '@tauri-apps/api/core';
import { importPreset, serializePreset, MAX_PRESET_BYTES } from '../../../utils/presets/transfer';
import { useEffect, useRef, useState } from 'react';
import { useEditorStore } from '../../../stores/editor';
import { captureStylePreset } from '../../../utils/presets/stylePreset';
import { loadPresetLibrary, savePresetLibrary, type PresetLibrary } from '../../../utils/presets/library';

export default function StylePresetControls({disabled=false}:{disabled?:boolean}){
  const input=useRef<HTMLInputElement>(null);
  const [notice,setNotice]=useState('');
  const [library,setLibrary]=useState<PresetLibrary|null>(null);
  const [selected,setSelected]=useState(''),[name,setName]=useState('');
  const [busy,setBusy]=useState(false),[error,setError]=useState('');
  const alive=useRef(true),pending=useRef(false);
  const preset=library?.presets.find(p=>p.id===selected);
  const reload=async()=>{
    if(pending.current)return;pending.current=true;setBusy(true);setError('');
    try {const loaded=await loadPresetLibrary();if(alive.current)setLibrary(loaded);}
    catch(error){if(alive.current)setError(String(error));}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  };
  useEffect(()=>{alive.current=true;void reload();return ()=>{alive.current=false;};},[]);
  const save=async(next:PresetLibrary,nextSelection=selected)=>{
    if(disabled || pending.current)return;pending.current=true;setBusy(true);setError('');
    try {const saved=await savePresetLibrary(next);if(alive.current){setLibrary(saved);setSelected(nextSelection);}}
    catch(error){if(alive.current)setError(String(error));}
    finally {pending.current=false;if(alive.current)setBusy(false);}
  };
  const capture=(replace=false)=>{
    if(!library)return;
    try {
      const captured=captureStylePreset(useEditorStore.getState(),replace && preset?preset.name:name,replace?preset?.id:undefined);
      void save({...library,presets:replace?library.presets.map(p=>p.id===captured.id?captured:p):[...library.presets,captured]},captured.id);
    }catch(error){setError(String(error));}
  };
  const importFile=async(file:File)=>{
    if(disabled || pending.current || !library)return;
    pending.current=true;setBusy(true);setError('');setNotice('');
    try {
      if(file.size>MAX_PRESET_BYTES)throw new Error('Preset files must be 4 MB or smaller.');
      const imported=importPreset(await file.text());
      if(!alive.current)return;
      const saved=await savePresetLibrary({...library,presets:[...library.presets,imported]});
      if(alive.current){setLibrary(saved);setSelected(imported.id);setName(imported.name);setNotice('Preset imported. Choose Apply style to use it.');}
    }catch(error){if(alive.current)setError(String(error));}
    finally{pending.current=false;if(alive.current)setBusy(false);if(input.current)input.current.value='';}
  };
  const exportFile=async()=>{
    if(disabled || pending.current || !preset)return;
    pending.current=true;setBusy(true);setError('');setNotice('');
    try {
      const path=await invoke<string>('export_style_preset',{preset:JSON.parse(serializePreset(preset))});
      if(alive.current)setNotice(`Preset exported to ${path}`);
    }catch(error){if(alive.current)setError(String(error));}
    finally{pending.current=false;if(alive.current)setBusy(false);}
  };
  return <div className="section">
    <div className="section-header"><h3>Style presets</h3><p>Reuse framing, motion, cursor, audio and export settings.</p></div>
    <input ref={input} hidden type="file" accept=".json,.tarantino-style.json" onChange={event=>{const file=event.target.files?.[0];if(file)void importFile(file);}}/>
    <button className="editor-btn editor-btn--ghost" disabled={disabled || busy || !library} onClick={()=>input.current?.click()}>Import preset</button>
    <div className="control-group"><label htmlFor="style-preset-name">Preset name</label>
      <input id="style-preset-name" value={name} maxLength={120} disabled={disabled || busy} onChange={e=>setName(e.target.value)}/></div>
    <button className="editor-btn editor-btn--ghost" disabled={disabled || busy || !library || !name.trim()} onClick={()=>capture()}>Save current style</button>
    <div className="control-group"><label htmlFor="saved-style">Saved styles</label>
      <select id="saved-style" value={selected} disabled={disabled || busy || !library} onChange={e=>{setSelected(e.target.value);setName(library?.presets.find(p=>p.id===e.target.value)?.name ?? '');}}>
        <option value="">Choose a preset</option>{library?.presets.map(p=><option key={p.id} value={p.id}>{p.name}{library.defaultId===p.id?' (default)':''}</option>)}
      </select></div>
    {preset && library && <label className="checkbox-control"><input type="checkbox"
      checked={library.defaultId===selected} disabled={disabled || busy}
      onChange={e=>void save({...library,defaultId:e.target.checked?selected:null})}/>
      Use for new recordings</label>}
    {preset && library && <div className="quality-presets">
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>void exportFile()}>Export preset</button>
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>useEditorStore.getState().applyStylePreset(preset)}>Apply style</button>
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>capture(true)}>Update from current style</button>
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy || !name.trim()} onClick={()=>void save({...library,presets:library.presets.map(p=>p.id===selected?{...p,name:name.trim()}:p)})}>Rename</button>
      <button className="editor-btn editor-btn--ghost" disabled={disabled || busy} onClick={()=>void save({...library,presets:library.presets.filter(p=>p.id!==selected),defaultId:library.defaultId===selected?null:library.defaultId},'')}>Remove preset</button>
    </div>}
    {busy && <p role="status">Working with presets…</p>}
    {notice && <p role="status" style={{overflowWrap:'anywhere'}}>{notice}</p>}
    {error && <p role="alert">{error}</p>}
    {error && <button className="editor-btn editor-btn--ghost" disabled={busy} onClick={()=>void reload()}>Reload presets</button>}
  </div>;
}
