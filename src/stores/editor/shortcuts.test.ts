import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {createProjectDocument,parseProjectDocument} from '../../utils/projects/document';
import {restoredProjectState} from '../../utils/projects/restore';
import {parseShortcutSettings} from '../../utils/shortcuts/settings';
import {shortcutIntervals} from '../../utils/shortcuts/intervals';
const initial=useEditorStore.getState();
beforeEach(()=>useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:5000,trimEnd:5000}));
it('undoes shortcut visibility, size, placement and hidden identities as project edits',()=>{
  const before=useEditorStore.getState().shortcutSettings;
  useEditorStore.getState().updateShortcutSettings({enabled:false,size:0.1,x:0.3,y:0.2,hidden:[100]});
  const edited=useEditorStore.getState().shortcutSettings;
  useEditorStore.getState().undo();
  expect(useEditorStore.getState().shortcutSettings).toEqual(before);
  useEditorStore.getState().redo();
  expect(useEditorStore.getState().shortcutSettings).toEqual(edited);
  const restored=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState()))));
  expect(restored.state.shortcutSettings).toEqual(edited);
});
it('migrates legacy settings without mutating the input and rejects malformed preferences',()=>{
  const legacy=createProjectDocument(useEditorStore.getState());Reflect.deleteProperty(legacy.state,'shortcutSettings');
  expect(parseProjectDocument(legacy).state.shortcutSettings).toEqual(parseShortcutSettings(undefined));
  expect(legacy.state).not.toHaveProperty('shortcutSettings');
  for(const patch of [{size:Infinity},{size:0},{x:-1},{y:2},{hidden:[NaN]},{enabled:'yes'}]) {
    const document=createProjectDocument(useEditorStore.getState());
    Object.assign(document.state.shortcutSettings,patch);
    expect(()=>parseProjectDocument(document)).toThrow('shortcut settings');
  }
});
it('restores settings while invalidating labels from the previous recording',()=>{
  const current=useEditorStore.getState();
  const document=createProjectDocument(current);
  useEditorStore.setState({shortcutIntervals:shortcutIntervals([{timestamp:100,key:'c',type:'down',modifiers:['command']}])});
  useEditorStore.setState(restoredProjectState(document,useEditorStore.getState()));
  expect(useEditorStore.getState().shortcutIntervals).toEqual([]);
  expect(useEditorStore.getState().shortcutSettings).toEqual(document.state.shortcutSettings);
});
