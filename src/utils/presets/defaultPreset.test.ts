import { beforeEach,expect,it } from 'vitest';
import { useEditorStore } from '../../stores/editor';
import { captureStylePreset } from './stylePreset';
import { applyDefaultPreset } from './defaultPreset';
import type { PresetLibrary } from './library';
const initial=useEditorStore.getState();
const preset=captureStylePreset({...initial,visualSettings:{...initial.visualSettings,padding:32}},'Default','default');
const library:PresetLibrary={version:1,revision:4,presets:[preset],defaultId:preset.id};
beforeEach(()=>{useEditorStore.setState({...initial,videoFilePath:'/new.mp4',recordingRevision:1,history:[],historyIndex:-1,historyGroup:null});});
it('initializes the default style without adding an undo step or modifying media',async()=>{
 const before=useEditorStore.getState();
 expect(await applyDefaultPreset(async()=>library)).toBe(true);
 const after=useEditorStore.getState();expect(after.visualSettings.padding).toBe(32);
 expect(after.clips).toBe(before.clips);expect(after.videoFilePath).toBe('/new.mp4');
 expect(after.history).toHaveLength(0);expect(after.canUndo).toBe(false);
 after.updateVisualSettings({padding:20});useEditorStore.getState().undo();
 expect(useEditorStore.getState().visualSettings.padding).toBe(32);
});
it.each(['project','visual','audio','timeline'] as const)('does not overwrite %s changes while loading',async(change)=>{
 let resolve!:(library:PresetLibrary)=>void;
 const pending=applyDefaultPreset(()=>new Promise(done=>{resolve=done;}));
 const current=useEditorStore.getState();
 if(change==='project')useEditorStore.setState({recordingRevision:2});
 if(change==='visual')current.updateVisualSettings({padding:11});
 if(change==='audio')current.updateAudioSettings({micGain:-4});
 if(change==='timeline')current.addOverlay({id:'one',type:'text',startTime:0,endTime:1000,x:0,y:0,properties:{}});
 const before=useEditorStore.getState();resolve(library);
 expect(await pending).toBe(false);expect(useEditorStore.getState().visualSettings).toBe(before.visualSettings);
 expect(useEditorStore.getState().history).toBe(before.history);
});
it('leaves settings intact when there is no default or the library fails',async()=>{
 const before=useEditorStore.getState();
 expect(await applyDefaultPreset(async()=>({...library,defaultId:null}))).toBe(false);
 await expect(applyDefaultPreset(async()=>{throw new Error('Unreadable library');})).rejects.toThrow('Unreadable');
 expect(useEditorStore.getState().visualSettings).toBe(before.visualSettings);
});
