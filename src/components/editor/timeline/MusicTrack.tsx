import { peakIndex } from '../../../utils/audio/peakIndex';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Music, Volume2, VolumeX } from 'lucide-react';
import { type BackgroundAudio } from '../../../utils/audio/backgroundAudio';
import { musicPeakRange } from '../../../utils/audio/musicWaveform';
import { useEditorStore } from '../../../stores/editor';

interface Props {music:BackgroundAudio;duration:number;pixelsPerMs:number;left:number;right:number;disabled:boolean}
export default memo(function MusicTrack({music,duration,pixelsPerMs,left,right,disabled}:Props){
  const canvas=useRef<HTMLCanvasElement>(null);
  const [data,setData]=useState<{peaks:number[];duration_ms:number}|null>(null);
  const [error,setError]=useState(false);
  useEffect(()=>{
    let current=true;setData(null);setError(false);
    void invoke<{peaks:number[];duration_ms:number}>('extract_audio_waveform_details',{audioPath:music.path,samplesPerSecond:100})
      .then(result=>{if(current)setData(result);}).catch(()=>{if(current)setError(true);});
    return ()=>{current=false;};
  },[music.path]);
  const start=Math.max(0,Math.floor(left)),end=Math.min(duration*pixelsPerMs,Math.ceil(right));
  const width=Math.max(0,end-start);
  const maximum=useMemo(()=>data ? peakIndex(data.peaks).maximum : 0,[data]);
  useEffect(()=>{
    const context=canvas.current?.getContext('2d');if(!context)return;
    context.clearRect(0,0,width,40);if(!data || pixelsPerMs<=0)return;
    context.fillStyle=music.muted?'#777':'#bd93f9';
    for(let x=0;x<width;x++){
      const level=musicPeakRange(data.peaks,data.duration_ms,(start+x)/pixelsPerMs,(start+x+1)/pixelsPerMs,maximum)*music.volume;
      context.fillRect(x,20-level*18,1,Math.max(1,level*36));
    }
  },[data,maximum,width,start,pixelsPerMs,music.volume,music.muted]);
  return <div className="timeline-track">
    <div className="track-header"><div className="track-label"><Music size={12}/><span>Music</span></div>
      <button className="track-toggle" disabled={disabled} aria-label={music.muted?'Unmute background music':'Mute background music'}
        onClick={()=>useEditorStore.getState().updateAudioSettings({backgroundAudio:{...music,muted:!music.muted}})}>
        {music.muted?<VolumeX size={12}/>:<Volume2 size={12}/>}</button></div>
    <div className="track-content" title={music.name}>
      <canvas ref={canvas} width={width} height={40} aria-label="Looping background music waveform"
        style={{position:'absolute',left:start,width,height:'100%',opacity:music.muted?0.4:0.85}}/>
      {(!data || error) && <span style={{position:'absolute',left:start+8,fontSize:11}}>{error?'Waveform unavailable':'Loading music waveform…'}</span>}
    </div>
  </div>;
});
