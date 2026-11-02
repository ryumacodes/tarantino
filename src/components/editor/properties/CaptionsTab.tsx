import {useShallow} from 'zustand/react/shallow';
import {useEditorStore} from '../../../stores/editor';
import type {CaptionSettings} from '../../../utils/captions/settings';
import type {TabProps} from './types';
import './captions.css';
import TranscriptEditor from './TranscriptEditor';
import CaptionGeneration from './CaptionGeneration';

export default function CaptionsTab({isExporting=false}:TabProps) {
  const {settings,hasTranscript,update}=useEditorStore(useShallow(state=>({
    settings:state.captionSettings,hasTranscript:state.transcripts.some(session=>session.words.length>0),
    update:state.updateCaptionSettings,
  })));
  const select=<K extends keyof CaptionSettings>(key:K,label:string,options:readonly (readonly [CaptionSettings[K],string])[],disabled=false)=>
    <label className="caption-field">{label}<select disabled={disabled} value={String(settings[key])}
      onChange={event=>update({[key]:event.target.value} as Pick<CaptionSettings,K>)}>
      {options.map(([value,title])=><option key={String(value)} value={String(value)}>{title}</option>)}
    </select></label>;
  return <div className="tab-content caption-controls">
    <CaptionGeneration isExporting={isExporting}/>
    {!hasTranscript&&<p className="setting-hint">This project has no transcript yet. Appearance settings will apply when a transcript is available.</p>}
    <TranscriptEditor isExporting={isExporting}/>
    <fieldset disabled={isExporting}>
      <legend className="caption-heading">Appearance</legend>
      <label className="caption-toggle"><input type="checkbox" checked={settings.enabled}
        onChange={event=>update({enabled:event.target.checked})}/> Show captions</label>
      {select('font','Font',[['sans-serif','Sans serif'],['serif','Serif'],['mono','Monospace']])}
      <label className="caption-field">Size · {Number((settings.sizeRatio*100).toFixed(1))}%
        <input aria-label="Caption size" type="range" min={2} max={12.5} step={.1} value={settings.sizeRatio*100}
          onChange={event=>update({sizeRatio:Number(event.target.value)/100})}/></label>
      <div className="caption-presets" aria-label="Caption color presets">
        <button type="button" aria-pressed={settings.color==='#ffffff'&&settings.backgroundColor==='#000000'}
          onClick={()=>update({color:'#ffffff',backgroundColor:'#000000'})}>Dark</button>
        <button type="button" aria-pressed={settings.color==='#000000'&&settings.backgroundColor==='#ffffff'}
          onClick={()=>update({color:'#000000',backgroundColor:'#ffffff'})}>Light</button>
      </div>
      <label className="caption-toggle"><input type="checkbox" checked={settings.showSoundLabels}
        onChange={event=>update({showSoundLabels:event.target.checked})}/> Include sound descriptions</label>
    </fieldset>
    <fieldset disabled={isExporting}>
      <legend className="caption-heading">Motion</legend>
      {select('wordsReveal','Reveal',[['line-by-line','Line by line'],['word-by-word','Word by word']])}
      {select('lineGrowth','Line growth',[['entire-line','Full line'],['left-anchored','From the left'],['centered','From the center']],settings.wordsReveal==='word-by-word'||settings.wordEntrance==='fade-in')}
      {select('wordEntrance','Word animation',[['appear','Appear'],['fade-in','Fade in'],['slide-in','Slide in']])}
      {settings.wordEntrance==='fade-in'&&settings.wordsReveal==='line-by-line'&&<p className="setting-hint">Upcoming words stay dimmed until spoken. Lines use their full width.</p>}
    </fieldset>
    <fieldset disabled={isExporting}>
      <legend className="caption-heading">Position</legend>
      <div className="caption-presets">
        <button type="button" onClick={()=>update({position:{x:.5,y:1}})}>Bottom</button>
        <button type="button" onClick={()=>update({position:{x:.5,y:.5}})}>Center</button>
      </div>
      {(['x','y'] as const).map(axis=><label className="caption-field" key={axis}>
        {axis==='x'?'Horizontal':'Vertical'} · {Math.round(settings.position[axis]*100)}%
        <input aria-label={`Caption ${axis==='x'?'horizontal':'vertical'} position`} type="range" min={0} max={100}
          value={settings.position[axis]*100} onChange={event=>update({position:{...settings.position,[axis]:Number(event.target.value)/100}})}/>
      </label>)}
    </fieldset>
  </div>;
}
