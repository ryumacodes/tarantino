import { beforeEach, expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { useEditorStore } from './store';
import { previewSample } from '../../utils/timeline/previewTimeline';
const initial = useEditorStore.getState();
beforeEach(() => {
  const base = {name:'Take',type:'video' as const,trackId:'v',enabled:true,locked:false,playbackRate:1};
  const clips = [
    {...base,id:'a',startTime:0,duration:100,sourceIn:0,sourceOut:100},
    {...base,id:'b',startTime:100,duration:200,sourceIn:300,sourceOut:700,playbackRate:2,micVolume:0.3,hideCursor:true},
    {...base,id:'c',startTime:300,duration:100,sourceIn:900,sourceOut:1000},
  ];
  useEditorStore.setState({...initial,videoFilePath:'/take.mp4',duration:1000,trimStart:0,trimEnd:400,currentTime:200,
    history:[],historyIndex:-1,clips,tracks:[{id:'v',name:'Video',type:'video',clips,height:60,visible:true,muted:false,solo:false,locked:false,order:0}]});
});
it('restores neighboring source gaps at the selected speed, ripples and preserves the source playhead', () => {
  const actions = useEditorStore.getState();
  const source = previewSample(actions, actions.currentTime).sourceTime;
  actions.resetClipTrim('b');
  let state = useEditorStore.getState();
  expect(state.clips[1]).toMatchObject({sourceIn:100,sourceOut:900,duration:400,playbackRate:2,micVolume:0.3,hideCursor:true});
  expect(state.clips[2].startTime).toBe(500);
  expect(state.trimEnd).toBe(600);
  expect(previewSample(state,state.currentTime).sourceTime).toBe(source);
  expect(state.tracks[0].clips).toEqual(state.clips);
  actions.undo(); state=useEditorStore.getState();
  expect(state.clips[1].sourceIn).toBe(300);
  expect(state.trimEnd).toBe(400);
  actions.redo(); expect(useEditorStore.getState().clips[1].sourceOut).toBe(900);
});
it.each(['start','end'] as const)('restores only the %s edge', side => {
  useEditorStore.getState().resetClipTrim('b',side);
  expect(useEditorStore.getState().clips[1]).toMatchObject({sourceIn:side==='start'?100:300,sourceOut:side==='end'?900:700,duration:300});
});
it('restores outer edges to recording bounds and includes restored footage in the export', () => {
  const clip = {...useEditorStore.getState().clips[1],startTime:0};
  useEditorStore.setState({clips:[clip],tracks:[{...useEditorStore.getState().tracks[0],clips:[clip]}],trimStart:0,trimEnd:200});
  useEditorStore.getState().resetClipTrim('b');
  expect(useEditorStore.getState().clips[0]).toMatchObject({sourceIn:0,sourceOut:1000,duration:500});
  expect(useEditorStore.getState().trimStart).toBe(0);
  expect(useEditorStore.getState().trimEnd).toBe(500);
});
it('does not edit locked clips or guess foreign media bounds', () => {
  useEditorStore.getState().updateClip('b',{locked:true});
  useEditorStore.getState().resetClipTrim('b');
  expect(useEditorStore.getState().clips[1].sourceIn).toBe(300);
  useEditorStore.getState().updateClip('b',{locked:false,sourceFilePath:'/other.mp4'});
  useEditorStore.getState().resetClipTrim('b');
  expect(useEditorStore.getState().clips[1].sourceIn).toBe(300);
});
it('trims a speed-adjusted clip, clamps the source playhead and ripples later footage', () => {
  const actions=useEditorStore.getState();
  actions.trimClip('b','start',500);
  let state=useEditorStore.getState();
  expect(state.clips[1]).toMatchObject({sourceIn:500,sourceOut:700,duration:100,startTime:100});
  expect(state.currentTime).toBe(100);
  expect(state.clips[2].startTime).toBe(200);
  expect(state.trimEnd).toBe(300);
  actions.trimClip('b','end',600);
  state=useEditorStore.getState();
  expect(state.clips[1].duration).toBe(50);
  expect(state.clips[2].startTime).toBe(150);
  expect(state.tracks[0].clips).toEqual(state.clips);
});
it('clamps edge drags to neighbors, keeps nonempty media and groups repeated moves into one undo', () => {
  const actions=useEditorStore.getState();
  actions.beginHistoryGroup();
  actions.trimClip('b','start',200);
  actions.trimClip('b','start',-500);
  actions.endHistoryGroup();
  expect(useEditorStore.getState().clips[1].sourceIn).toBe(100);
  actions.undo();
  expect(useEditorStore.getState().clips[1].sourceIn).toBe(300);
  expect(useEditorStore.getState().canUndo).toBe(false);
  actions.trimClip('b','end',10000);
  expect(useEditorStore.getState().clips[1].sourceOut).toBe(900);
  actions.trimClip('b','end',-100);
  expect(useEditorStore.getState().clips[1]).toMatchObject({sourceOut:400,duration:50});
});
it('rejects nonfinite trim targets and locked-track edits', () => {
  const actions=useEditorStore.getState();
  actions.trimClip('b','end',NaN);
  expect(useEditorStore.getState().history).toHaveLength(0);
  actions.updateTrack('v',{locked:true});
  actions.trimClip('b','end',500);
  expect(useEditorStore.getState().clips[1].sourceOut).toBe(700);
});
