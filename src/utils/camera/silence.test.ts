import { expect,it,vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { loadCameraSilence,inSilence } from './silence';
import { useEditorStore } from '../../stores/editor';
import { sampleWebcam } from './webcamTimeline';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const ranges=[{start_ms:1000,end_ms:6000},{start_ms:9000,end_ms:10000}];
it('uses half-open source-time intervals and leaves fullscreen camera visible',()=>{
 const visual=useEditorStore.getState().visualSettings;
 for(const [time,quiet] of [[999,false],[1000,true],[5999,true],[6000,false],[9000,true],[10000,false]] as const){
  expect(inSilence(ranges,time)).toBe(quiet);
  expect(sampleWebcam(visual,[],time,ranges).visible).toBe(!quiet);
  expect(sampleWebcam({...visual,webcamLayout:'fullscreen'},[],time,ranges).visible).toBe(true);
 }
 expect(sampleWebcam(visual,[{id:'full',time:2000,x:0.5,y:0.5,size:0.2,visible:true,shape:'circle',layout:'fullscreen'}],3000,ranges).visible).toBe(true);
});
it('shares preview/export analysis but refreshes when the recording or duration setting changes',async()=>{
 vi.mocked(invoke).mockReset();vi.mocked(invoke).mockResolvedValue(ranges);
 const state={...useEditorStore.getState(),videoFilePath:'/takes/processed_demo.mp4',recordingRevision:700,hasMicrophone:true,hasWebcam:true,visualSettings:{...useEditorStore.getState().visualSettings,webcamHideSilenceMs:5000}};
 const first=loadCameraSilence(state),second=loadCameraSilence(state);expect(first).toBe(second);expect(await first).toEqual(ranges);
 expect(invoke).toHaveBeenCalledWith('detect_audio_silence',{audioPath:'/takes/demo.mic.wav',minimumMs:5000,threshold:0.1});
 await loadCameraSilence({...state,recordingRevision:701});await loadCameraSilence({...state,visualSettings:{...state.visualSettings,webcamHideSilenceMs:1000}});
 expect(invoke).toHaveBeenCalledTimes(3);
 await loadCameraSilence({...state,hasMicrophone:false});await loadCameraSilence({...state,visualSettings:{...state.visualSettings,webcamHideSilenceMs:null}});
 expect(invoke).toHaveBeenCalledTimes(3);
});
it('allows retry after analysis errors',async()=>{
 vi.mocked(invoke).mockReset();vi.mocked(invoke).mockRejectedValueOnce(new Error('Missing microphone')).mockResolvedValue(ranges);
 const state={...useEditorStore.getState(),videoFilePath:'/retry.mp4',recordingRevision:800,hasMicrophone:true,hasWebcam:true,visualSettings:{...useEditorStore.getState().visualSettings,webcamHideSilenceMs:5000}};
 await expect(loadCameraSilence(state)).rejects.toThrow('Missing microphone');
 expect(await loadCameraSilence(state)).toEqual(ranges);
});
it('maps camera silence through the edited source clock after a cut and speed change',async()=>{
 const {previewSample}=await import('../timeline/previewTimeline');
 const visual=useEditorStore.getState().visualSettings;
 const state={duration:10000,tracks:[{id:'video',type:'video',visible:true,solo:false,muted:false,order:0}],clips:[{id:'cut',trackId:'video',type:'video',enabled:true,startTime:0,duration:2000,sourceIn:4000,sourceOut:8000,playbackRate:2}]};
 const at=(time:number)=>sampleWebcam(visual,[],previewSample(state as any,time).sourceTime!,ranges).visible;
 expect(at(0)).toBe(false);expect(at(999)).toBe(false);expect(at(1000)).toBe(true);
});
