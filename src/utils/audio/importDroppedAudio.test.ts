import { beforeEach, expect, it, vi } from 'vitest';
import { useEditorStore } from '../../stores/editor';
import { droppedAudioPath, importDroppedAudio } from './importDroppedAudio';
const initial=useEditorStore.getState();
const oldMusic={path:'/old.wav',name:'old',volume:0.3,muted:true};
const asset={path:'/owned/music.wav',name:'music.wav'};
beforeEach(()=>useEditorStore.setState({...initial,recordingRevision:1,audioSettings:{...initial.audioSettings,backgroundAudio:oldMusic}}));
it('accepts supported files case-insensitively and ignores unrelated drops',()=>{
 expect(droppedAudioPath(['/video.mp4','/Music.FLAC','/other.wav'])).toBe('/Music.FLAC');
 expect(droppedAudioPath(['/music.wav.exe','/folder'])).toBeUndefined();
});
it('adds owned music preserving volume, unmutes it, and supports undo',async()=>{
 expect(await importDroppedAudio('/external.wav',async()=>asset,()=>true)).toBe(true);
 expect(useEditorStore.getState().audioSettings.backgroundAudio).toEqual({...asset,volume:0.3,muted:false});
 useEditorStore.getState().undo();
 expect(useEditorStore.getState().audioSettings.backgroundAudio).toEqual(oldMusic);
});
it.each(['recording','music','disabled'] as const)('rejects late imports after %s changes',async(change)=>{
 let resolve!:(value:typeof asset)=>void;
 let enabled=true;
 const pending=importDroppedAudio('/external.wav',()=>new Promise(done=>{resolve=done;}),()=>enabled);
 if(change==='recording')useEditorStore.setState({recordingRevision:2});
 if(change==='music')useEditorStore.getState().updateAudioSettings({backgroundAudio:undefined});
 if(change==='disabled')enabled=false;
 const before=useEditorStore.getState().audioSettings.backgroundAudio;
 resolve(asset);expect(await pending).toBe(false);
 expect(useEditorStore.getState().audioSettings.backgroundAudio).toBe(before);
});
it('keeps existing music when native validation fails',async()=>{
 const failure=new Error('No readable audio');
 await expect(importDroppedAudio('/broken.wav',vi.fn().mockRejectedValue(failure),()=>true)).rejects.toThrow(failure);
 expect(useEditorStore.getState().audioSettings.backgroundAudio).toEqual(oldMusic);
});
