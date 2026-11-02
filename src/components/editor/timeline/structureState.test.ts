import { beforeEach, expect, it, vi } from 'vitest';
import { shallow } from 'zustand/shallow';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import { useEditorStore } from '../../../stores/editor';
import { timelineStructureState } from './structureState';
import { newSourceZoomRange } from '../../../utils/timeline/sourceRanges';
import { removablePreviousClip } from '../../../utils/timeline/previousClip';
const initial=useEditorStore.getState();
beforeEach(()=>useEditorStore.setState(initial));

it('keeps track inputs stable across 1200 real playback updates while clocks keep advancing',()=>{
  let fullUpdates=0, structureUpdates=0, clockUpdates=0;
  const unsubscribe=useEditorStore.subscribe((state,previous)=>{
    fullUpdates++;
    if(!shallow(timelineStructureState(state),timelineStructureState(previous))) structureUpdates++;
    if(state.currentTime !== previous.currentTime) clockUpdates++;
  });
  for(let frame=1;frame<=1200;frame++) useEditorStore.getState().setCurrentTime(frame*1000/120,'playback');
  unsubscribe();
  expect(fullUpdates).toBe(1200);
  expect(clockUpdates).toBe(1200);
  expect(structureUpdates).toBe(0);
  expect(useEditorStore.getState().currentTime).toBe(10000);
});

it('still updates tracks for edits and loading, while seek and play state stay clock-only',()=>{
  const snapshot=()=>timelineStructureState(useEditorStore.getState());
  const before=snapshot();
  useEditorStore.getState().setCurrentTime(500);
  useEditorStore.getState().setIsPlaying(true);
  expect(shallow(before,snapshot())).toBe(true);
  useEditorStore.setState({thumbnails:['frame']});
  expect(shallow(before,snapshot())).toBe(false);
  const loaded=snapshot();
  useEditorStore.getState().setTrimStart(100);
  expect(shallow(loaded,snapshot())).toBe(false);
});

it('updates edit availability at clip boundaries even when structure stays stable',()=>{
  const clip={id:'a',name:'A',type:'video' as const,trackId:'v',startTime:0,duration:2000,
    sourceIn:0,sourceOut:2000,playbackRate:1,enabled:true,locked:false};
  const clips=[clip,{...clip,id:'b',startTime:2000,sourceIn:2000,sourceOut:4000}];
  useEditorStore.setState({duration:4000,clips,tracks:[{id:'v',name:'Video',type:'video',clips,
    height:60,visible:true,muted:false,solo:false,locked:false,order:0}],currentTime:0});
  const before=timelineStructureState(useEditorStore.getState());
  expect(removablePreviousClip(useEditorStore.getState(),0)).toBeNull();
  useEditorStore.getState().setCurrentTime(2200,'playback');
  expect(removablePreviousClip(useEditorStore.getState(),2200)?.id).toBe('a');
  expect(newSourceZoomRange(useEditorStore.getState(),2200)).not.toBeNull();
  useEditorStore.getState().setCurrentTime(4000,'playback');
  expect(newSourceZoomRange(useEditorStore.getState(),4000)).toBeNull();
  expect(shallow(before,timelineStructureState(useEditorStore.getState()))).toBe(true);
});
