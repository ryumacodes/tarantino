import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {createProjectDocument,parseProjectDocument} from '../../utils/projects/document';
import {restoredProjectState} from '../../utils/projects/restore';
import type {RecordingMask} from '../../utils/masks/document';
const initial=useEditorStore.getState();
const mask:RecordingMask={id:'private',sourceFilePath:'/take.mp4',sourceStartMs:1000,sourceEndMs:4000,
 coordinateSpace:{x:200,y:100,width:400,height:200,sourceWidth:800,sourceHeight:400},
 disabled:false,type:'sensitive-data',bounds:[{x:10,y:20,width:100,height:50}],highlightOpacity:.5};
beforeEach(()=>useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:5000,trimEnd:5000}));
it('round-trips source coverage and supports undo/redo of edits and removal',()=>{
 useEditorStore.getState().setMasks([mask]);
 useEditorStore.getState().setMasks([{...mask,type:'highlight',highlightOpacity:.7}]);
 useEditorStore.getState().undo();expect(useEditorStore.getState().masks).toEqual([mask]);
 useEditorStore.getState().redo();
 const document=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState()))));
 useEditorStore.setState({masks:[]});useEditorStore.setState(restoredProjectState(document,useEditorStore.getState()));
 expect(useEditorStore.getState().masks).toEqual([{...mask,type:'highlight',highlightOpacity:.7}]);
 useEditorStore.getState().setMasks([]);useEditorStore.getState().undo();expect(useEditorStore.getState().masks).toHaveLength(1);
});
it('rejects invalid replacement atomically and defaults legacy projects without mutating input',()=>{
 useEditorStore.getState().setMasks([mask]);
 const before=useEditorStore.getState();
 expect(()=>before.setMasks([{...mask,bounds:[]}])).toThrow();
 expect(useEditorStore.getState()).toBe(before);
 const document=createProjectDocument(before);Reflect.deleteProperty(document.state,'masks');
 expect(parseProjectDocument(document).state.masks).toEqual([]);expect(document.state).not.toHaveProperty('masks');
 document.state.masks=[{...mask,bounds:[]}];expect(()=>parseProjectDocument(document)).toThrow('masks');
});
it('clears masks on a new recording',()=>{
 useEditorStore.getState().setMasks([mask]);
 useEditorStore.setState({loadThumbnails:vi.fn(),loadPreviewZoomData:vi.fn(),loadZoomData:vi.fn()});
 useEditorStore.getState().initializeEditor('/next.mp4',1000);
 expect(useEditorStore.getState().masks).toEqual([]);
});
it('groups mask creation and bulk edits into undoable actions',()=>{
 const state=useEditorStore.getState();
 const input={sourceFilePath:'/take.mp4',sourceTimeMs:0,sourceDurationMs:5000,
  coordinateSpace:mask.coordinateSpace!,type:'highlight' as const,highlightOpacity:.7};
 const first=state.addMask(input);expect(first.created).toBe(true);
 const history=useEditorStore.getState().history;
 expect(state.addMask(input).created).toBe(false);expect(useEditorStore.getState().history).toBe(history);
 const second=state.addMask({...input,sourceTimeMs:3000,highlightOpacity:.4});
 state.applyMaskOpacityToAll(first.mask.id);expect(useEditorStore.getState().masks.map(m=>m.highlightOpacity)).toEqual([.7,.7]);
 state.undo();expect(useEditorStore.getState().masks.map(m=>m.highlightOpacity)).toEqual([.7,.4]);
 const before=useEditorStore.getState();
 expect(()=>state.updateMask(first.mask.id,{sourceEndMs:4000})).toThrow('overlaps');
 expect(useEditorStore.getState()).toBe(before);
 state.disableAllMasks();expect(useEditorStore.getState().masks.every(m=>m.disabled)).toBe(true);
 state.undo();expect(useEditorStore.getState().masks.every(m=>!m.disabled)).toBe(true);
 state.removeMask(second.mask.id);expect(useEditorStore.getState().masks).toHaveLength(1);
 state.undo();expect(useEditorStore.getState().masks).toHaveLength(2);
 state.removeAllMasks();expect(useEditorStore.getState().masks).toHaveLength(0);
 state.undo();expect(useEditorStore.getState().masks).toHaveLength(2);
});
