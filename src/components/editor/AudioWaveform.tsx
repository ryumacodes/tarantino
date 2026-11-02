import { memo, useRef, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../stores/editor';
import { recordedWaveform } from '../../utils/audio/recordedWaveform';
import { channelGain } from '../../utils/audio/previewAudio';

interface Props {
  type:'microphone'|'system'; duration:number; pixelsPerMs:number;
  audioPath?:string|null; viewportLeft:number; viewportRight:number;
}
export default memo(function AudioWaveform({type,duration,pixelsPerMs,audioPath,viewportLeft,viewportRight}:Props){
  const canvas=useRef<HTMLCanvasElement>(null);
  const [data,setData]=useState<{path:string;peaks:number[]}|null>(null);
  const [error,setError]=useState<string|null>(null);
  const clips=useEditorStore(s=>s.clips),tracks=useEditorStore(s=>s.tracks);
  const sourceDuration=useEditorStore(s=>s.duration),sourcePath=useEditorStore(s=>s.videoFilePath);
  const settings=useEditorStore(s=>s.audioSettings);
  useEffect(()=>{
    let active=true;setData(null);setError(null);
    if(!audioPath)return;
    void invoke<number[]>('extract_audio_waveform',{audioPath,samplesPerSecond:100})
      .then(peaks=>{if(active)setData({path:audioPath,peaks});})
      .catch(()=>{if(active)setError('Waveform unavailable');});
    return ()=>{active=false;};
  },[audioPath]);
  const start=Math.max(0,Math.floor(viewportLeft));
  const width=Math.max(0,Math.ceil(Math.min(duration*pixelsPerMs,viewportRight))-start);
  const gain=channelGain(settings,type);
  useEffect(()=>{
    const context=canvas.current?.getContext('2d');if(!context)return;
    context.clearRect(0,0,width,40);
    if(!data || data.path!==audioPath)return;
    const columns=recordedWaveform({clips,tracks,duration:sourceDuration},sourcePath,data.peaks,type,pixelsPerMs,start,start+width,gain);
    context.fillStyle=type==='microphone'?'#50fa7b':'#8be9fd';
    columns.forEach((peak,x)=>{if(peak>0)context.fillRect(x,20-peak*18,1,Math.max(1,peak*36));});
  },[data,audioPath,clips,tracks,sourceDuration,sourcePath,type,pixelsPerMs,start,width,gain]);
  return <>
    <canvas ref={canvas} width={width} height={40} aria-label={`${type} waveform`}
      style={{position:'absolute',left:start,width,height:'100%',opacity:0.8}}/>
    {audioPath && (!data || error) && <span style={{position:'absolute',left:start+8,fontSize:11}}>{error ?? 'Loading waveform…'}</span>}
  </>;
});
