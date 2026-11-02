import {expect,it} from 'vitest';
import {useEditorStore} from '../../stores/editor';
import {shortcutMotionSource,createShortcutMotion} from './motionTimeline';
import {shortcutIntervals} from './intervals';
const base=()=>({...useEditorStore.getState(),videoFilePath:'/main.mp4',duration:4000,trimStart:100,trimEnd:3500,
 tracks:[{id:'v',name:'Video',type:'video' as const,clips:[],height:60,order:0,visible:true,solo:false,muted:false,locked:false}],
 clips:[{id:'fast',name:'fast',type:'video' as const,trackId:'v',startTime:0,sourceIn:1000,sourceOut:2000,duration:500,playbackRate:2,enabled:true,locked:false}]});
it('maps source time and rejects gaps, foreign sources and trim endpoints',()=>{
 const state=base();
 expect(shortcutMotionSource(state,0)?.time).toBe(1200);
 expect(shortcutMotionSource(state,399)?.time).toBe(1998);
 for(const t of [-1,400,3400,NaN,Infinity])expect(shortcutMotionSource(state,t)).toBeNull();
 expect(shortcutMotionSource({...state,clips:state.clips.map(c=>({...c,sourceFilePath:'/other.mp4'}))},0)).toBeNull();
});
it('returns fullscreen suppression without discarding source geometry',()=>{
 const state=base();state.hasWebcam=true;state.visualSettings={...state.visualSettings,webcamEnabled:true};
 state.webcamKeyframes=[{id:'full',time:1000,x:.5,y:.5,size:.2,shape:'circle',visible:true,layout:'fullscreen'}];
 expect(shortcutMotionSource(state,0)).toEqual({time:1200,fullscreenCamera:true});
});
it('removes hidden instances before animation construction',()=>{
 const state=base();state.shortcutIntervals=shortcutIntervals([{timestamp:1100,type:'down',key:'c',modifiers:['command']}]);
 state.shortcutSettings={...state.shortcutSettings,hidden:[1100]};
 expect(createShortcutMotion(state,1000,600,60).sample(100)).toEqual([]);
});
