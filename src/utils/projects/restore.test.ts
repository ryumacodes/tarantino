import { beforeEach,expect,it,vi } from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../stores/editor';
import { createProjectDocument } from './document';
import { restoredProjectState } from './restore';
const initial=useEditorStore.getState();
beforeEach(()=>{
  vi.mocked(invoke).mockReset();
  useEditorStore.setState({...initial,videoFilePath:'/take.mp4',recordingRevision:4});
});
it('restores saved edits and capture geometry while clearing runtime state',()=>{
  const state=useEditorStore.getState();
  const saved=createProjectDocument({...state,projectTitle:'Saved cut',captureMode:'window',
    screenResolution:{width:1470,height:956},scaleFactor:2,trimStart:100,trimEnd:500});
  useEditorStore.setState({isPlaying:true,thumbnails:['old'],thumbnailsLoading:true,
    mouseEventsLoading:true,previewZoomLoading:true,currentTime:900});
  useEditorStore.setState(restoredProjectState(saved,useEditorStore.getState()));
  expect(useEditorStore.getState()).toMatchObject({projectTitle:'Saved cut',captureMode:'window',
    screenResolution:{width:1470,height:956},scaleFactor:2,trimStart:100,trimEnd:500,
    recordingRevision:5,isPlaying:false,currentTime:0,thumbnails:[],thumbnailsLoading:false,
    mouseEventsLoading:false,previewZoomLoading:false,canUndo:false});
});
it.each(['resolve','reject'] as const)('a same-file project restore ignores an older zoom request that will %s',async(outcome)=>{
  let resolve!:(value:unknown)=>void,reject!:(reason:Error)=>void;
  vi.mocked(invoke).mockReturnValue(new Promise((yes,no)=>{resolve=yes;reject=no;}));
  const state=useEditorStore.getState();
  const saved=createProjectDocument({...state,zoomAnalysis:{zoom_blocks:[],total_clicks:42,session_duration:state.duration,
    config:{enabled:true,zoom_factor:2,zoom_duration:1000,min_click_spacing:1000}}});
  const pending=state.loadZoomData('/take.mp4');
  useEditorStore.setState(restoredProjectState(saved,useEditorStore.getState()));
  if(outcome==='resolve')resolve({zoom_blocks:[],total_clicks:0}); else reject(new Error('stale'));
  await pending;
  expect(useEditorStore.getState().zoomAnalysis).toEqual(saved.state.zoomAnalysis);
  expect(useEditorStore.getState().zoomLoading).toBe(false);
});
it.each(['resolve','reject'] as const)('preserves a zoom added while background analysis is still loading (%s)',async(outcome)=>{
  let resolve!:(value:unknown)=>void,reject!:(reason:Error)=>void;
  vi.mocked(invoke).mockReturnValue(new Promise((yes,no)=>{resolve=yes;reject=no;}));
  const log=vi.spyOn(console,'error').mockImplementation(()=>{});
  const state=useEditorStore.getState();
  const pending=state.loadZoomData('/take.mp4');
  state.addZoomBlock({id:'manual',start_time:100,end_time:2000,zoom_factor:2,center_x:0.5,center_y:0.5,click_x:0.5,click_y:0.5,is_manual:true});
  const edited=useEditorStore.getState().zoomAnalysis;
  if(outcome==='resolve')resolve({zoom_blocks:[],total_clicks:0}); else reject(new Error('unavailable'));
  await pending;
  expect(useEditorStore.getState().zoomAnalysis).toEqual(edited);
  expect(useEditorStore.getState().zoomLoading).toBe(false);
  log.mockRestore();
});
