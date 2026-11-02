import { beforeEach, expect, it, vi } from 'vitest';
import { shallow } from 'zustand/shallow';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import { useEditorStore } from '../../stores/editor';
import { selectEditorShellInputs } from './shellInputs';
const initial=useEditorStore.getState();
beforeEach(()=>useEditorStore.setState(initial));

it('keeps the editor shell stable through playback, cursor samples, and clip selection',()=>{
  const before=selectEditorShellInputs(useEditorStore.getState());
  for(let frame=1;frame<=1200;frame++) {
    useEditorStore.getState().setCurrentTime(frame*1000/120,'playback');
    expect(shallow(before,selectEditorShellInputs(useEditorStore.getState()))).toBe(true);
  }
  useEditorStore.setState({mouseEvents:[],selection:{clipIds:['a'],trackIds:[],keyframeIds:[]}});
  expect(shallow(before,selectEditorShellInputs(useEditorStore.getState()))).toBe(true);
});
it('invalidates for playback start/stop and project identity while leaving edits to their own subscribers',()=>{
  const read=()=>selectEditorShellInputs(useEditorStore.getState());
  const before=read();
  useEditorStore.getState().setIsPlaying(true);
  expect(shallow(before,read())).toBe(false);
  const playing=read();
  useEditorStore.getState().setIsPlaying(false);
  expect(shallow(playing,read())).toBe(false);
  const stopped=read();
  useEditorStore.setState({projectTitle:'New title',videoFilePath:'/new.mp4'});
  expect(shallow(stopped,read())).toBe(false);
  const loaded=read();
  useEditorStore.getState().updateAudioSettings({micGain:.5});
  useEditorStore.getState().setTrimStart(100);
  expect(shallow(loaded,read())).toBe(true);
  expect(useEditorStore.getState().audioSettings.micGain).toBe(.5);
  expect(useEditorStore.getState().trimStart).toBe(100);
});
