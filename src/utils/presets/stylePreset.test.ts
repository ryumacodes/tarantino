import { beforeEach,expect,it } from 'vitest';
import { useEditorStore } from '../../stores/editor';
import { captureStylePreset,parseStylePreset } from './stylePreset';
const initial=useEditorStore.getState();
beforeEach(()=>useEditorStore.setState({...initial,history:[],historyIndex:-1,historyGroup:null}));
it('captures only reusable settings and round-trips without media timeline or crop',()=>{
 const state=useEditorStore.getState();
 const preset=captureStylePreset({...state,videoFilePath:'/private/take.mp4',visualSettings:{...state.visualSettings,sourceCrop:{x:0,y:0,width:100,height:100}},audioSettings:{...state.audioSettings,backgroundAudio:{path:'/music.wav',name:'Music',volume:1,muted:false}}},'  My style  ','style-id');
 expect(preset.name).toBe('My style');
 expect(JSON.stringify(preset)).not.toContain('/private');expect(JSON.stringify(preset)).not.toContain('/music.wav');
 expect(preset.visual).not.toHaveProperty('sourceCrop');
 expect(preset).not.toHaveProperty('clips');
 expect(parseStylePreset(JSON.parse(JSON.stringify(preset)))).toEqual(preset);
});
it('applies all settings as one undoable edit while preserving recording-specific state',()=>{
 const state=useEditorStore.getState();
 const preset=captureStylePreset({...state,visualSettings:{...state.visualSettings,padding:31,cursorScale:2},audioSettings:{...state.audioSettings,micGain:-6},exportSettings:{...state.exportSettings,frameRate:60}},'Saved','id');
 const sourceCrop={x:0,y:0,width:100,height:100};
 const music={path:'/current.wav',name:'Current music',volume:0.4,muted:false};
 useEditorStore.setState({visualSettings:{...state.visualSettings,sourceCrop},audioSettings:{...state.audioSettings,backgroundAudio:music},videoFilePath:'/current.mp4',currentTime:1234,isPlaying:true});
 const before=useEditorStore.getState();
 before.applyStylePreset(preset);
 const after=useEditorStore.getState();
 expect(after.visualSettings.padding).toBe(31);expect(after.audioSettings.micGain).toBe(-6);expect(after.exportSettings.frameRate).toBe(60);
 expect(after.visualSettings.sourceCrop).toEqual(sourceCrop);expect(after.audioSettings.backgroundAudio).toEqual(music);
 expect(after.videoFilePath).toBe('/current.mp4');expect(after.currentTime).toBe(1234);expect(after.clips).toBe(before.clips);
 expect(after.isPlaying).toBe(false);expect(after.playbackRevision).toBe(before.playbackRevision+1);
 expect(after.history).toHaveLength(2);
 after.undo();expect(useEditorStore.getState().visualSettings).toEqual(before.visualSettings);expect(useEditorStore.getState().audioSettings).toEqual(before.audioSettings);expect(useEditorStore.getState().exportSettings).toEqual(before.exportSettings);
 useEditorStore.getState().redo();expect(useEditorStore.getState().visualSettings.padding).toBe(31);
});
it('rejects invalid presets without creating an edit',()=>{
 const preset=captureStylePreset(useEditorStore.getState(),'Valid','id');
 for(const invalid of [{...preset,version:2},{...preset,name:''},{...preset,visual:{...preset.visual,cursorScale:NaN}},{...preset,visual:{...preset.visual,cursorStyle:'invalid'}},{...preset,visual:{...preset.visual,gradientStops:[{color:'red',position:Infinity}]}},{...preset,output:{...preset.output,resolution:'custom',customWidth:0}}]){
  expect(()=>useEditorStore.getState().applyStylePreset(invalid as any)).toThrow('Invalid preset');
 }
 expect(useEditorStore.getState().history).toHaveLength(0);
});
it('preserves absent legacy motion modes and strips unknown properties',()=>{
 const preset=captureStylePreset(useEditorStore.getState(),'Legacy','id');
 delete preset.visual.screenMotionStyle;delete preset.visual.cursorMotionStyle;
 const parsed=parseStylePreset({...preset,visual:{...preset.visual,unexpected:'value'}});
 expect(parsed.visual.screenMotionStyle).toBeUndefined();expect(parsed.visual.cursorMotionStyle).toBeUndefined();
 expect(parsed.visual).not.toHaveProperty('unexpected');
});
it('preserves camera silence duration in styles and rejects invalid durations',()=>{
 const state=useEditorStore.getState();
 const preset=captureStylePreset({...state,visualSettings:{...state.visualSettings,webcamHideSilenceMs:5000}},'Silence','id');
 expect(parseStylePreset(preset).visual.webcamHideSilenceMs).toBe(5000);
 expect(()=>parseStylePreset({...preset,visual:{...preset.visual,webcamHideSilenceMs:-1}})).toThrow();
});
it('preserves camera and zoom visibility while rejecting malformed values',()=>{
 const state=useEditorStore.getState();
 const preset=captureStylePreset({...state,visualSettings:{...state.visualSettings,webcamEnabled:false,zoomEffectsEnabled:false}},'Hidden','id');
 expect(parseStylePreset(JSON.parse(JSON.stringify(preset))).visual).toMatchObject({webcamEnabled:false,zoomEffectsEnabled:false});
 for(const key of ['webcamEnabled','zoomEffectsEnabled']) expect(()=>parseStylePreset({...preset,visual:{...preset.visual,[key]:'false'}})).toThrow(key);
});
