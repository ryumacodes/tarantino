import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {applyGeneratedTranscript} from './generatedTranscript';
const initial=useEditorStore.getState();
const words=[{id:'0',text:'Hello',startMs:50,endMs:200,type:'word' as const}];
beforeEach(()=>useEditorStore.setState({...initial,videoFilePath:'/take.mp4',recordingRevision:1}));
it('rejects a late result after switching projects, even at the same path',()=>{
 const origin=useEditorStore.getState();
 useEditorStore.setState({recordingRevision:2});
 expect(applyGeneratedTranscript(origin,'/take.mp4',words)).toBe(false);
 expect(useEditorStore.getState().transcripts).toEqual([]);
});
it('replaces rather than overlaps a transcript and supports undo',()=>{
 const origin=useEditorStore.getState();
 expect(applyGeneratedTranscript(origin,'/take.mp4',words)).toBe(true);
 const id=useEditorStore.getState().transcripts[0].id;
 expect(applyGeneratedTranscript(origin,'/take.mp4',[{...words[0],text:'Again'}])).toBe(true);
 expect(useEditorStore.getState().transcripts).toHaveLength(1);
 expect(useEditorStore.getState().transcripts[0].id).toBe(id);
 useEditorStore.getState().undo();
 expect(useEditorStore.getState().transcripts[0].words).toEqual(words);
});
it('does not attach results for a removed recording',()=>{
 expect(applyGeneratedTranscript(useEditorStore.getState(),'/removed.mp4',words)).toBe(false);
});
it('retains recognition language through regeneration and undo',()=>{
 const origin=useEditorStore.getState();
 applyGeneratedTranscript(origin,'/take.mp4',words,{type:'system',locale:'fr_FR'});
 applyGeneratedTranscript(origin,'/take.mp4',words,{type:'system',locale:'en_US'});
 expect(useEditorStore.getState().transcripts[0].generator?.locale).toBe('en_US');
 useEditorStore.getState().undo();
 expect(useEditorStore.getState().transcripts[0].generator?.locale).toBe('fr_FR');
});
