import { expect,it } from 'vitest';
import { recordedWaveform } from './recordedWaveform';
import type { TimelineClip,TimelineTrack } from '../../stores/editor/types';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const clip=(id:string,startTime:number,sourceIn:number,sourceOut:number,playbackRate=1):TimelineClip=>
 ({id,name:id,type:'video',trackId:'v',startTime,sourceIn,sourceOut,playbackRate,duration:(sourceOut-sourceIn)/playbackRate,enabled:true,locked:false});
const peaks=Array.from({length:400},(_,i)=>i<100?0.2:i<200?0.4:i<300?0.6:0.8);
const rounded=(data:Float32Array)=>Array.from(data,n=>Math.round(n*100)/100);
it('reorders and retimes recorded peaks instead of stretching the source',()=>{
 const state={duration:4000,tracks:[track],clips:[clip('late',0,2000,4000,2),clip('early',1000,0,1000,0.5)]};
 expect(rounded(recordedWaveform(state,'/take',peaks,'microphone',0.002,0,6))).toEqual([0.6,0.8,0.2,0.2,0.2,0.2]);
});
it('leaves gaps and foreign sources empty and honors per-channel and track gain',()=>{
 const state={duration:4000,tracks:[track],clips:[{...clip('one',0,1000,2000),micVolume:0.5,systemVolume:0.25},{...clip('foreign',2000,0,1000),sourceFilePath:'/other'}]};
 expect(rounded(recordedWaveform(state,'/take',peaks,'microphone',0.001,0,3,2))).toEqual([0.4,0,0]);
 expect(rounded(recordedWaveform(state,'/take',peaks,'system',0.001,0,3,2))).toEqual([0.2,0,0]);
 expect(rounded(recordedWaveform({...state,tracks:[{...track,muted:true}]},'/take',peaks,'microphone',0.001,0,3))).toEqual([0,0,0]);
});
it('retains a brief peak in a pixel spanning a cut',()=>{
 const source=Array(100).fill(0);source[10]=0.9;
 const state={duration:1000,tracks:[track],clips:[clip('tiny',0,100,110),clip('rest',10,500,990)]};
 expect(rounded(recordedWaveform(state,'/take',source,'microphone',0.002,0,1))).toEqual([0.9]);
});
it('uses the visible layer and does not fall through hidden or solo tracks',()=>{
 const state={duration:4000,tracks:[track,{...track,id:'front',order:-1,solo:true}],clips:[clip('back',0,0,1000),{...clip('front',0,3000,4000),trackId:'front'}]};
 expect(rounded(recordedWaveform(state,'/take',peaks,'system',0.001,0,1))).toEqual([0.8]);
 expect(rounded(recordedWaveform({...state,tracks:state.tracks.map(t=>({...t,visible:false}))},'/take',peaks,'system',0.001,0,1))).toEqual([0]);
});
it('allocates only the viewport for long legacy recordings and handles empty data',()=>{
 const state={duration:3600000,tracks:[],clips:[]};
 expect(recordedWaveform(state,'/take',[], 'system',1,1000000,1000800)).toHaveLength(800);
 expect(rounded(recordedWaveform({duration:4000,tracks:[],clips:[]},'/take',peaks,'system',0.001,1,3))).toEqual([0.4,0.6]);
 expect(recordedWaveform(state,'/take',peaks,'system',0,0,100)).toHaveLength(0);
});
