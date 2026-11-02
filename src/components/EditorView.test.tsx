import { beforeEach, expect, it, vi } from 'vitest';
const mocks=vi.hoisted(()=>({invoke:vi.fn(),flagUpdates:[] as number[][]}));
vi.mock('zustand/react/shallow',()=>({useShallow:(selector:any)=>selector}));
vi.mock('@tauri-apps/api/core',()=>({invoke:mocks.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock('../utils/camera/silence',()=>({loadCameraSilence:vi.fn(async()=>[])}));
vi.mock('react',async()=>{
  const actual=await vi.importActual<typeof import('react')>('react');
  const hooks={useState:(initial:any)=>[initial,(update:any)=>{
    if(Array.isArray(initial)) mocks.flagUpdates.push(typeof update==='function'?update(initial):update);
  }],useEffect:()=>{},useRef:(value:any)=>({current:value}),useCallback:(callback:any)=>callback};
  return {...actual,...hooks,default:{...actual,...hooks}};
});
vi.mock('../stores/editor',async()=>{
  const actual=await vi.importActual<typeof import('../stores/editor')>('../stores/editor');
  return {...actual,useEditorStore:Object.assign((selector:any)=>selector(actual.useEditorStore.getState()),actual.useEditorStore)};
});
import EditorView from './EditorView';
import { useEditorStore } from '../stores/editor/store';
const initial=useEditorStore.getState();
beforeEach(()=>{
  useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:5000,trimEnd:5000});
  mocks.invoke.mockReset().mockResolvedValue('/export.mp4');mocks.flagUpdates=[];
});
function findProp(node:any,name:string):any {
  if(!node || typeof node!=='object') return undefined;
  if(node.props?.[name]) return node.props[name];
  for(const child of [node.props?.children].flat(Infinity)) {
    const found=findProp(child,name);if(found) return found;
  }
}
it('exports the latest trim and audio settings even if the shell has not rerendered',async()=>{
  const tree=EditorView({onClose:()=>{}});
  useEditorStore.setState({trimStart:500,trimEnd:3500,
    audioSettings:{...initial.audioSettings,micGain:.4,micMuted:true}});
  await findProp(tree,'onExport')();
  const call=mocks.invoke.mock.calls.find(([name])=>name==='export_video');
  expect(call).toBeDefined();
  expect(call![1].settings.trim_start).toBe(500);
  expect(call![1].settings.trim_end).toBe(3500);
  expect(call![1].settings.audio_settings.mic_gain).toBe(.4);
  expect(call![1].settings.audio_settings.mic_muted).toBe(true);
});
it('captures a flag at action time rather than the last shell render',()=>{
  const tree=EditorView({onClose:()=>{}});
  useEditorStore.getState().setCurrentTime(2345,'playback');
  findProp(tree,'onAction')('flag');
  expect(mocks.flagUpdates).toEqual([[2345]]);
});
