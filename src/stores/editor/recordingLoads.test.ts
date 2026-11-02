import { beforeEach,expect,it,vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from './store';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const initial=useEditorStore.getState();
const deferred = () => {
  let resolve!:(value:unknown)=>void, reject!:(error:Error)=>void;
  const promise=new Promise<unknown>((yes,no)=>{resolve=yes;reject=no;});
  return {promise,resolve,reject};
};
beforeEach(()=>{
  vi.mocked(invoke).mockReset();
  useEditorStore.setState({...initial,videoFilePath:'/a.mp4',recordingRevision:1,duration:3000});
});
const cases = [
  {load:'loadThumbnails',field:'thumbnails',loading:'thumbnailsLoading',old:['old'],fresh:['new'],arg:'/a.mp4'},
  {load:'loadZoomData',field:'zoomAnalysis',loading:'zoomLoading',old:{zoom_blocks:[],total_clicks:1},fresh:{zoom_blocks:[],total_clicks:2},arg:'/a.mp4'},
  {load:'loadPreviewZoomData',field:'previewZoomAnalysis',loading:'previewZoomLoading',old:{id:'old'},fresh:{id:'new'},arg:'/a.mp4'},
  {load:'loadMouseEvents',field:'mouseEvents',loading:'mouseEventsLoading',old:'[]',fresh:JSON.stringify([{timestamp:500,x:12,y:24,event_type:'Move'}]),arg:'/a.mouse.json'},
] as const;
it.each(cases)('$load ignores an older result after the newer request completes',async(test)=>{
  const older=deferred(),newer=deferred();
  vi.mocked(invoke).mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
  const actions=useEditorStore.getState();
  const a=actions[test.load](test.arg), b=actions[test.load](test.arg);
  newer.resolve(test.fresh); await b;
  const expected=useEditorStore.getState()[test.field];
  older.resolve(test.old); await a;
  expect(useEditorStore.getState()[test.field]).toEqual(expected);
  expect(useEditorStore.getState()[test.loading]).toBe(false);
});
it.each(cases)('$load ignores stale failures while another recording request remains pending',async(test)=>{
  const older=deferred(),newer=deferred();
  vi.mocked(invoke).mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
  const actions=useEditorStore.getState();
  const a=actions[test.load](test.arg);
  useEditorStore.setState({videoFilePath:'/b.mp4',recordingRevision:2});
  const b=actions[test.load](test.arg.replace('/a.','/b.'));
  older.reject(new Error('old failed')); await a;
  expect(useEditorStore.getState()[test.loading]).toBe(true);
  newer.resolve(test.fresh);await b;
  expect(useEditorStore.getState()[test.loading]).toBe(false);
});
it('invalidates mouse metadata even when the same recording is reopened',async()=>{
  const request=deferred();
  vi.mocked(invoke).mockImplementation(command=>command==='read_sidecar_file'?request.promise:Promise.resolve(command==='extract_video_thumbnails'?[]:null));
  const pending=useEditorStore.getState().loadMouseEvents('/a.mouse.json');
  useEditorStore.setState({audioSettings:{...useEditorStore.getState().audioSettings,backgroundAudio:{path:'/old.wav',name:'Old',volume:1,muted:false}},isPlaying:true,displayResolution:{width:999,height:999},captureMode:'window',scaleFactor:2});
  useEditorStore.getState().initializeEditor('/a.mp4',3000);
  request.resolve(JSON.stringify({display_width:100,display_height:100,mouse_events:[]}));
  await pending;
  expect(useEditorStore.getState().audioSettings.backgroundAudio).toBeUndefined();
  expect(useEditorStore.getState()).toMatchObject({recordingRevision:2,mouseEvents:null,mouseEventsLoading:false,
    isPlaying:false,displayResolution:null,captureMode:null,scaleFactor:1});
});
it('does not start thumbnail work for a file that is no longer selected',async()=>{
  await useEditorStore.getState().loadThumbnails('/other.mp4');
  expect(invoke).not.toHaveBeenCalled();
});
it('loads explicitly captured shortcuts and clears them when opening a legacy sidecar',async()=>{
  vi.mocked(invoke).mockResolvedValueOnce(JSON.stringify({mouse_events:[],key_events:[
    {timestamp:100,is_release:false,is_typing:false,shortcut:{key:'c',modifiers:['command'],repeat:false}},
    {timestamp:200,is_release:true,is_typing:false,shortcut:{key:'c',modifiers:[],repeat:false}},
  ]}));
  await useEditorStore.getState().loadMouseEvents('/a.mouse.json');
  expect(useEditorStore.getState().shortcutIntervals).toMatchObject([{key:'c',firstDown:100,lastEvent:200}]);
  vi.mocked(invoke).mockResolvedValueOnce(JSON.stringify({mouse_events:[],key_events:[{timestamp:100,is_typing:true}]}));
  await useEditorStore.getState().loadMouseEvents('/a.mouse.json');
  expect(useEditorStore.getState().shortcutIntervals).toEqual([]);
});
