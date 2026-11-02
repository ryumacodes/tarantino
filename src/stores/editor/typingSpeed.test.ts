import {beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import {useEditorStore} from './store';
import {createProjectDocument,parseProjectDocument} from '../../utils/projects/document';
const initial=useEditorStore.getState();
beforeEach(()=>{
  const clip={id:'take',name:'Take',type:'video' as const,trackId:'v',startTime:0,duration:6000,
    sourceIn:0,sourceOut:6000,playbackRate:1,enabled:true,locked:false,micVolume:0.4,hideCursor:true};
  useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:6000,trimEnd:6000,currentTime:4000,
    clips:[clip],tracks:[{id:'v',name:'Video',type:'video',clips:[clip],height:60,visible:true,muted:false,solo:false,locked:false,order:0}]});
});
it('splits only the selected source section and stores one undoable speed edit',()=>{
  useEditorStore.getState().speedUpTypingRange('take',1000,4000,3);
  let state=useEditorStore.getState();
  expect(state.clips.map(c=>[c.sourceIn,c.sourceOut,c.startTime,c.duration,c.playbackRate])).toEqual([
    [0,1000,0,1000,1],[1000,4000,1000,1000,3],[4000,6000,2000,2000,1]]);
  expect(state.tracks[0].clips).toEqual(state.clips);
  expect(state.clips.every(c=>c.micVolume===0.4 && c.hideCursor)).toBe(true);
  expect(state.trimEnd).toBe(4000); expect(state.currentTime).toBe(2000);
  const saved=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(state))));
  expect(saved.state.clips).toEqual(state.clips);
  state.undo(); state=useEditorStore.getState();
  expect(state.clips).toHaveLength(1); expect(state.trimEnd).toBe(6000);expect(state.canUndo).toBe(false);
  state.redo();expect(useEditorStore.getState().clips).toHaveLength(3);
});
it('does not split locked clips or invalid ranges',()=>{
  for (const args of [[-1,4000,3],[1000,7000,3],[4000,1000,3],[1000,4000,NaN],[1000,4000,9]]) {
    useEditorStore.getState().speedUpTypingRange('take',...args as [number,number,number]);
    expect(useEditorStore.getState().clips).toHaveLength(1);
  }
  useEditorStore.setState(state=>({tracks:state.tracks.map(track=>({...track,locked:true}))}));
  useEditorStore.getState().speedUpTypingRange('take',1000,4000,3);
  expect(useEditorStore.getState().clips).toHaveLength(1);
  expect(useEditorStore.getState().canUndo).toBe(false);
});

it('applies multiple ranges across repeated source clips as one undoable edit',()=>{
  const first=useEditorStore.getState().clips[0];
  const second={...first,id:'repeat',startTime:6000};
  useEditorStore.setState(state=>({duration:12000,trimEnd:12000,clips:[first,second],
    tracks:[{...state.tracks[0],clips:[first,second]}],
    typingRanges:[{start:500,end:2000},{start:3000,end:4500}]}));
  useEditorStore.getState().speedUpAllTypingRanges(3);
  const state=useEditorStore.getState();
  expect(state.clips.filter(c=>c.playbackRate===3).map(c=>[c.sourceIn,c.sourceOut,c.startTime]))
    .toEqual([[500,2000,500],[3000,4500,2000],[500,2000,4500],[3000,4500,6000]]);
  expect(state.trimEnd).toBe(8000);
  expect(state.tracks[0].clips).toEqual(state.clips);
  state.undo();
  expect(useEditorStore.getState().clips).toEqual([first,second]);
  expect(useEditorStore.getState().trimEnd).toBe(12000);
  expect(useEditorStore.getState().canUndo).toBe(false);
});

it('persists dismissal, supports undo, and reads older projects without dismissals',()=>{
  useEditorStore.getState().dismissTypingRange(1000);
  const saved=createProjectDocument(useEditorStore.getState());
  expect(parseProjectDocument(JSON.parse(JSON.stringify(saved))).state.dismissedTypingRanges).toEqual([1000]);
  useEditorStore.getState().undo();
  expect(useEditorStore.getState().dismissedTypingRanges).toEqual([]);
  const legacy=JSON.parse(JSON.stringify(saved));
  delete legacy.state.dismissedTypingRanges;
  expect(parseProjectDocument(legacy).state.dismissedTypingRanges).toEqual([]);
  expect(legacy.state).not.toHaveProperty('dismissedTypingRanges');
  saved.state.dismissedTypingRanges=[-1];
  expect(()=>parseProjectDocument(saved)).toThrow('dismissed typing ranges');
});

it('excludes dismissed suggestions from bulk apply',()=>{
  useEditorStore.setState({typingRanges:[{start:500,end:2000},{start:3000,end:4500}],dismissedTypingRanges:[500]});
  useEditorStore.getState().speedUpAllTypingRanges(3);
  expect(useEditorStore.getState().clips.filter(c=>c.playbackRate===3).map(c=>c.sourceIn)).toEqual([3000]);
});

it('offers typing in speed-adjusted clips and resolves it without losing undo', async()=>{
  const {typingSpeedSuggestions}=await import('../../utils/timeline/typingSpeedSuggestions');
  useEditorStore.getState().setClipPlaybackRate('take',2);
  useEditorStore.setState({typingRanges:[{start:1000,end:4000}]});
  expect(typingSpeedSuggestions(useEditorStore.getState())).toHaveLength(1);
  useEditorStore.getState().speedUpAllTypingRanges(3);
  let state=useEditorStore.getState();
  expect(state.clips.map(c=>[c.sourceIn,c.sourceOut,c.startTime,c.duration,c.playbackRate])).toEqual([
    [0,1000,0,500,2],[1000,4000,500,1000,3],[4000,6000,1500,1000,2]]);
  expect(state.dismissedTypingRanges).toEqual([1000]);
  expect(typingSpeedSuggestions(state)).toEqual([]);
  state.undo();state=useEditorStore.getState();
  expect(state.clips).toHaveLength(1);
  expect(state.clips[0].playbackRate).toBe(2);
  expect(typingSpeedSuggestions(state)).toHaveLength(1);
});

it('resolves an already matching speed without introducing redundant cuts',()=>{
  useEditorStore.getState().setClipPlaybackRate('take',3);
  useEditorStore.setState({typingRanges:[{start:1000,end:4000}]});
  useEditorStore.getState().speedUpTypingRange('take',1000,4000,3);
  expect(useEditorStore.getState().clips).toHaveLength(1);
  expect(useEditorStore.getState().dismissedTypingRanges).toEqual([1000]);
  useEditorStore.getState().undo();
  expect(useEditorStore.getState().dismissedTypingRanges).toEqual([]);
});
