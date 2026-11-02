import {beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {useEditorStore} from './store';
import {mergeRebuiltZooms} from './rebuildZooms';
import type {ZoomAnalysis,ZoomBlock} from './types';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const initial=useEditorStore.getState();
const block=(id:string,start:number,end:number,manual=false)=>({id,start_time:start,end_time:end,is_manual:manual,
  zoom_factor:2,center_x:0.5,center_y:0.5,click_x:0.5,click_y:0.5} as ZoomBlock);
const analysis=(blocks:ZoomBlock[])=>({zoom_blocks:blocks,total_clicks:2,session_duration:10000,
  config:{enabled:true,zoom_factor:2,zoom_duration:5000,min_click_spacing:500}} as ZoomAnalysis);
beforeEach(()=>{vi.mocked(invoke).mockReset();useEditorStore.setState({...initial,videoFilePath:'/a.mp4',recordingRevision:1});});
it('preserves manual edits and automatic coverage on both sides',()=>{
 const manual=block('manual',3000,5000,true);
 const result=mergeRebuiltZooms(analysis([block('old',0,9000)]),analysis([manual]));
 expect(result.zoom_blocks.map(b=>[b.start_time,b.end_time])).toEqual([[0,3000],[3000,5000],[5000,9000]]);
 expect(result.zoom_blocks[1]).toBe(manual);
 expect(new Set(result.zoom_blocks.map(b=>b.id)).size).toBe(3);
});
it('applies one undoable edit without saving shared analysis',async()=>{
 const original=analysis([block('old',0,2000)]);
 useEditorStore.setState({zoomAnalysis:original});
 vi.mocked(invoke).mockResolvedValue(analysis([block('new',3000,8000)]));
 expect(await useEditorStore.getState().regenerateZooms()).toBe(true);
 expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks[0].start_time).toBe(3000);
 useEditorStore.getState().undo();
 expect(useEditorStore.getState().zoomAnalysis).toEqual(original);
 expect(vi.mocked(invoke).mock.calls.map(call=>call[0])).toEqual(['regenerate_auto_zoom_data']);
});
it('does not overwrite edits or another project when analysis finishes late',async()=>{
 for(const changedProject of [false,true]) {
  useEditorStore.setState({...initial,videoFilePath:'/a.mp4',recordingRevision:1,zoomAnalysis:analysis([])});
  let resolve!:(value:ZoomAnalysis)=>void;
  vi.mocked(invoke).mockReturnValue(new Promise<ZoomAnalysis>(yes=>{resolve=yes;}));
  const pending=useEditorStore.getState().regenerateZooms();
  const edited=analysis([block('kept',1000,4000,true)]);
  useEditorStore.setState({zoomAnalysis:edited,...(changedProject?{videoFilePath:'/b.mp4',recordingRevision:2}:{})});
  resolve(analysis([block('new',0,9000)]));
  if(changedProject) expect(await pending).toBe(false);
  else await expect(pending).rejects.toThrow('Zooms changed');
  expect(useEditorStore.getState().zoomAnalysis).toEqual(edited);
 }
});
