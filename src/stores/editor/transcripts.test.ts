import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {createProjectDocument,parseProjectDocument} from '../../utils/projects/document';
import {restoredProjectState} from '../../utils/projects/restore';
import {parseTranscripts,type TranscriptSession} from '../../utils/captions/document';
const initial=useEditorStore.getState();
const session:TranscriptSession={id:'session',sourceFilePath:'/take.mp4',offsetMs:1000,
 words:[{id:'word',text:'hello',startMs:100,endMs:500,type:'word'}]};
beforeEach(()=>useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:5000,trimEnd:5000}));
it('persists and restores edits with original timing and supports undo/redo',()=>{
 useEditorStore.getState().setTranscriptSession(session);
 useEditorStore.getState().updateTranscriptText('session','word','Hello!');
 expect(useEditorStore.getState().transcripts[0].words[0].text).toBe('Hello!');
 useEditorStore.getState().undo();expect(useEditorStore.getState().transcripts).toEqual([session]);
 useEditorStore.getState().redo();
 const document=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState()))));
 useEditorStore.setState({transcripts:[]});
 useEditorStore.setState(restoredProjectState(document,useEditorStore.getState()));
 expect(useEditorStore.getState().transcripts[0].words[0]).toEqual({...session.words[0],text:'Hello!'});
 expect(useEditorStore.getState().transcripts[0].offsetMs).toBe(1000);
});
it('defaults older projects without mutating them and rejects corrupt transcript data',()=>{
 const doc=createProjectDocument(useEditorStore.getState());Reflect.deleteProperty(doc.state,'transcripts');
 expect(parseProjectDocument(doc).state.transcripts).toEqual([]);expect(doc.state).not.toHaveProperty('transcripts');
 for(const invalid of [[session,session],[{...session,offsetMs:NaN}],[{...session,words:[session.words[0],session.words[0]]}],
 [{...session,words:[{...session.words[0],endMs:-1}]}]])expect(()=>parseTranscripts(invalid)).toThrow('transcripts');
});
it('does not retain captions from the previous recording',()=>{
 useEditorStore.getState().setTranscriptSession(session);
 useEditorStore.setState({loadThumbnails:vi.fn(),loadPreviewZoomData:vi.fn(),loadZoomData:vi.fn()});
 useEditorStore.getState().initializeEditor('/next.mp4',1000);
 expect(useEditorStore.getState().transcripts).toEqual([]);
});
it('saves word visibility and restores it through undo',()=>{
 useEditorStore.getState().setTranscriptSession(session);
 useEditorStore.getState().setTranscriptWordHidden('session','word',true);
 const document=createProjectDocument(useEditorStore.getState());
 expect(parseProjectDocument(document).state.transcripts[0].words[0].isHidden).toBe(true);
 useEditorStore.getState().undo();expect(useEditorStore.getState().transcripts[0].words[0].isHidden).toBeUndefined();
 expect(()=>parseTranscripts([{...session,words:[{...session.words[0],isHidden:'yes'}]}])).toThrow('transcripts');
});
it('keeps session ordering when recognition data is replaced',()=>{
 useEditorStore.getState().setTranscriptSession(session);
 useEditorStore.getState().setTranscriptSession({...session,id:'later',offsetMs:3000});
 useEditorStore.getState().setTranscriptSession({...session,words:[{...session.words[0],text:'updated'}]});
 expect(useEditorStore.getState().transcripts.map(x=>x.id)).toEqual(['session','later']);
});
it('round-trips generator metadata while preserving older transcripts',()=>{
 const withGenerator={...session,generator:{type:'system' as const,locale:'fr_FR'}};
 useEditorStore.getState().setTranscriptSession(withGenerator);
 const document=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState()))));
 expect(document.state.transcripts[0].generator).toEqual(withGenerator.generator);
 expect(parseTranscripts([session])[0]).not.toHaveProperty('generator');
 for(const generator of [{type:'other',locale:'en_US'},{type:'system',locale:''},{type:'system',locale:1},null])
  expect(()=>parseTranscripts([{...session,generator}])).toThrow('transcripts');
});
