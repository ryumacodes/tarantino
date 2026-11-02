import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {createProjectDocument,parseProjectDocument} from '../../utils/projects/document';
import {parseCaptionSettings} from '../../utils/captions/settings';
const initial=useEditorStore.getState();
beforeEach(()=>useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:5000,trimEnd:5000}));
it('undoes caption appearance and preserves settings through project round trips',()=>{
 const before=useEditorStore.getState().captionSettings;
 useEditorStore.getState().updateCaptionSettings({font:'mono',sizeRatio:.08,position:{x:.3,y:.8},
  wordEntrance:'fade-in',wordsReveal:'word-by-word',showSoundLabels:true,color:'#ffeedd',backgroundColor:'#12345680'});
 const edited=useEditorStore.getState().captionSettings;
 useEditorStore.getState().undo();expect(useEditorStore.getState().captionSettings).toEqual(before);
 useEditorStore.getState().redo();expect(useEditorStore.getState().captionSettings).toEqual(edited);
 expect(parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState())))).state.captionSettings).toEqual(edited);
});
it('defaults legacy projects and keeps default position objects independent',()=>{
 const document=createProjectDocument(useEditorStore.getState());Reflect.deleteProperty(document.state,'captionSettings');
 expect(parseProjectDocument(document).state.captionSettings).toEqual(parseCaptionSettings(undefined));
 expect(document.state).not.toHaveProperty('captionSettings');
 const a=parseCaptionSettings(undefined),b=parseCaptionSettings(undefined);a.position.x=0;
 expect(b.position.x).toBe(.5);
});
it('rejects invalid colors, placement, size and modes before they reach the renderer',()=>{
 for(const patch of [{enabled:'yes'},{sizeRatio:Infinity},{sizeRatio:0},{font:'unknown'},{color:'url(secret)'},
 {backgroundColor:'#nope'},{position:{x:-1,y:1}},{wordEntrance:'bounce'},{lineGrowth:'other'},{wordsReveal:'other'}])
  expect(()=>parseCaptionSettings({...parseCaptionSettings(undefined),...patch})).toThrow('caption settings');
});
it('clears old appearance on a new recording',()=>{
 useEditorStore.getState().updateCaptionSettings({enabled:false});
 useEditorStore.setState({loadThumbnails:vi.fn(),loadPreviewZoomData:vi.fn(),loadZoomData:vi.fn()});
 useEditorStore.getState().initializeEditor('/next.mp4',1000);
 expect(useEditorStore.getState().captionSettings).toEqual(parseCaptionSettings(undefined));
});
