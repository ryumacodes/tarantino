import "./shortcuts.css";
import {useState} from 'react';
import {useShallow} from 'zustand/react/shallow';
import {useEditorStore} from '../../../stores/editor';
import {shortcutLabels} from '../../../utils/shortcuts/keycaps';
import type {TabProps} from './types';

export default function ShortcutsTab({isExporting=false}:TabProps) {
  const {settings,intervals,update}=useEditorStore(useShallow(state=>({settings:state.shortcutSettings,
    intervals:state.shortcutIntervals,update:state.updateShortcutSettings})));
  const [page,setPage]=useState(0);
  const lastPage=Math.max(0,Math.ceil(intervals.length/40)-1),current=Math.min(page,lastPage);
  return <div className="tab-content shortcut-controls">
    <div className="section">
      <div className="section-header"><h3>Keyboard shortcuts</h3><p>Recorded key combinations</p></div>
      {!intervals.length?<p className="setting-hint">No shortcut labels in this recording. Enable “Record keyboard shortcuts” before your next recording.</p>:<>
        <div className="control-group"><label><input type="checkbox" checked={settings.enabled} disabled={isExporting}
          onChange={e=>update({enabled:e.target.checked})}/> Show shortcuts</label></div>
        <div className="control-group"><label htmlFor="shortcut-size">Size: {Number((settings.size*100).toFixed(1))}%</label>
          <input id="shortcut-size" type="range" min={3} max={15} step={0.5} value={settings.size*100} disabled={isExporting}
            onChange={e=>update({size:Number(e.target.value)/100})}/></div>
        <div className="control-group"><label htmlFor="shortcut-x">Horizontal position: {Math.round(settings.x*100)}%</label>
          <input id="shortcut-x" type="range" min={0} max={100} value={settings.x*100} disabled={isExporting}
            onChange={e=>update({x:Number(e.target.value)/100})}/></div>
        <div className="control-group"><label htmlFor="shortcut-y">Vertical position: {Math.round(settings.y*100)}%</label>
          <input id="shortcut-y" type="range" min={0} max={100} value={settings.y*100} disabled={isExporting}
            onChange={e=>update({y:Number(e.target.value)/100})}/></div>
        <div className="section-header"><h3>Recorded shortcuts</h3><p>Uncheck a shortcut to hide it everywhere it appears.</p></div>
        {intervals.slice(current*40,(current+1)*40).map(interval=><div className="control-group" key={interval.firstDown}>
          <label style={{display:'flex',alignItems:'center',gap:8}}><input type="checkbox" disabled={isExporting}
            checked={!settings.hidden.includes(interval.firstDown)} onChange={e=>update({hidden:e.target.checked?
              settings.hidden.filter(time=>time!==interval.firstDown):[...settings.hidden,interval.firstDown]})}/>
            <span>{shortcutLabels(interval).join(' ')}</span>
            <small style={{marginLeft:'auto'}}>{(interval.firstDown/1000).toFixed(1)} s</small>
          </label></div>)}
        {lastPage>0&&<div className="control-group" style={{display:'flex',gap:8}}>
          <button disabled={current===0} onClick={()=>setPage(current-1)}>Previous</button>
          <span>{current+1} / {lastPage+1}</span>
          <button disabled={current===lastPage} onClick={()=>setPage(current+1)}>Next</button>
        </div>}
      </>}
    </div>
  </div>;
}
