import { readZoomStyle } from '../../utils/motion/zoomStylePreference';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { splitTimelineAtTime } from '../../utils/timeline/splitTimeline';
import { previewSample } from '../../utils/timeline/previewTimeline';
import { exportTimeline } from '../../utils/timeline/exportTimeline';
import { useEditorStore } from './store';
const initial = useEditorStore.getState();
beforeEach(() => {
  const clip = { id:'take',name:'Take',type:'video' as const,trackId:'v',startTime:0,duration:2000,
    sourceIn:0,sourceOut:2000,playbackRate:1,enabled:true,locked:false };
  useEditorStore.setState({ ...initial, videoFilePath:'/take.mp4',duration:2000,trimEnd:2000,
    history:[],historyIndex:-1,canUndo:false,canRedo:false,clips:[clip],
    tracks:[{id:'v',name:'Video',type:'video',clips:[clip],height:60,visible:true,muted:false,solo:false,locked:false,order:0}] });
});
it('undoes the first audio edit and redoes it without restoring playback or history itself', () => {
  const actions=useEditorStore.getState();
  actions.updateClip('take',{micVolume:0.3,systemVolume:0.7});
  expect(useEditorStore.getState().canUndo).toBe(true);
  actions.setCurrentTime(800); actions.setIsPlaying(true);
  actions.undo();
  let state=useEditorStore.getState();
  expect(state.clips[0].micVolume).toBeUndefined();
  expect(state.tracks[0].clips[0].micVolume).toBeUndefined();
  expect(state.currentTime).toBe(800); expect(state.isPlaying).toBe(false);
  expect(state.canUndo).toBe(false); expect(state.canRedo).toBe(true);
  actions.redo(); state=useEditorStore.getState();
  expect(state.clips[0]).toMatchObject({micVolume:0.3,systemVolume:0.7});
  expect(state.tracks[0].clips[0]).toEqual(state.clips[0]);
  expect(state.historyIndex).toBe(1); expect(state.canRedo).toBe(false);
  for(const entry of state.history) for(const excluded of ['history','historyIndex','currentTime','thumbnails','mouseEvents','updateClip']) {
    expect(entry).not.toHaveProperty(excluded);
  }
});
it('preserves clip properties through split, delete, undo, redo and a new branch', () => {
  const actions=useEditorStore.getState();
  actions.updateClip('take',{micVolume:0.4,systemVolume:0,hideCursor:true});
  const split=actions.cutClip('take',1000)!;
  expect(useEditorStore.getState().clips[1]).toMatchObject({micVolume:0.4,systemVolume:0,hideCursor:true,sourceIn:1000});
  actions.deleteClip(split); actions.undo();
  expect(useEditorStore.getState().clips).toHaveLength(2);
  actions.undo();
  expect(useEditorStore.getState().clips).toHaveLength(1);
  expect(useEditorStore.getState().clips[0].duration).toBe(2000);
  actions.redo();
  expect(useEditorStore.getState().clips).toHaveLength(2);
  actions.updateClip(split,{micVolume:0.8});
  expect(useEditorStore.getState().canRedo).toBe(false);
  actions.redo();
  expect(useEditorStore.getState().clips[1].micVolume).toBe(0.8);
});
it('shares unchanged immutable data, bounds retained edits and excludes playback/no-op updates', () => {
  const actions=useEditorStore.getState();
  actions.updateClip('missing',{micVolume:0.3});
  actions.setCurrentTime(900); actions.selectClips(['take']);
  expect(useEditorStore.getState().history).toHaveLength(0);
  const visual=useEditorStore.getState().visualSettings;
  for(let n=0;n<130;n++) actions.setProjectTitle(`Take ${n}`);
  const state=useEditorStore.getState();
  expect(state.history).toHaveLength(101);
  expect(state.historyIndex).toBe(100);
  expect(state.history[0].visualSettings).toBe(visual);
  expect(state.history[100].visualSettings).toBe(visual);
  for(let n=0;n<100;n++) actions.undo();
  expect(useEditorStore.getState().canUndo).toBe(false);
});
it('restores visual, global audio, trim and camera edits without reverting async caches', () => {
  const actions=useEditorStore.getState();
  actions.updateAudioSettings({micGain:-6});
  actions.setTrimStart(200);
  actions.addWebcamKeyframe({id:'cam',time:500,x:0.2,y:0.3,size:0.2,shape:'circle',visible:true});
  useEditorStore.setState({thumbnails:['new-thumbnail'],mouseEvents:[]});
  actions.undo(); expect(useEditorStore.getState().webcamKeyframes).toHaveLength(0);
  actions.undo(); expect(useEditorStore.getState().trimStart).toBe(0);
  actions.undo(); expect(useEditorStore.getState().audioSettings.micGain).toBe(0);
  expect(useEditorStore.getState().thumbnails).toEqual(['new-thumbnail']);
});

it('coalesces hundreds of gesture updates and keeps successive gestures independent', () => {
  const actions=useEditorStore.getState();
  actions.beginHistoryGroup();
  for(let n=1;n<=200;n++) actions.updateClip('take',{micVolume:n/200});
  actions.endHistoryGroup();
  expect(useEditorStore.getState().history).toHaveLength(2);
  actions.beginHistoryGroup();
  actions.updateClip('take',{systemVolume:0.5});
  actions.updateClip('take',{systemVolume:0.2});
  actions.endHistoryGroup();
  actions.undo();
  expect(useEditorStore.getState().clips[0]).toMatchObject({micVolume:1});
  expect(useEditorStore.getState().clips[0].systemVolume).toBeUndefined();
  actions.undo();
  expect(useEditorStore.getState().clips[0].micVolume).toBeUndefined();
  actions.redo(); actions.redo();
  expect(useEditorStore.getState().clips[0]).toMatchObject({micVolume:1,systemVolume:0.2});
});
it('nested groups keep combined cuts atomic, and empty groups leave redo intact', () => {
  const actions=useEditorStore.getState();
  const second={...useEditorStore.getState().clips[0],id:'second'};
  actions.addClip(second);
  actions.beginHistoryGroup();
  actions.cutClipsAtTime(1000);
  actions.setTrimEnd(1800);
  actions.endHistoryGroup();
  expect(useEditorStore.getState().history).toHaveLength(3);
  expect(useEditorStore.getState().clips).toHaveLength(4);
  actions.undo();
  expect(useEditorStore.getState().clips).toHaveLength(2);
  expect(useEditorStore.getState().trimEnd).toBe(2000);
  actions.beginHistoryGroup(); actions.endHistoryGroup();
  expect(useEditorStore.getState().canRedo).toBe(true);
  actions.redo(); expect(useEditorStore.getState().clips).toHaveLength(4);
});
it('undo interrupts an open gesture so later edits create a fresh branch', () => {
  const actions=useEditorStore.getState();
  actions.beginHistoryGroup(); actions.updateClip('take',{micVolume:0.4});
  actions.undo();
  expect(useEditorStore.getState().historyGroup).toBeNull();
  actions.updateClip('take',{micVolume:0.8}); actions.endHistoryGroup();
  expect(useEditorStore.getState().canRedo).toBe(false);
  actions.undo(); expect(useEditorStore.getState().clips[0].micVolume).toBeUndefined();
});

it('ripples speed edits, preserves source intervals and remaps explicit trim/playhead', () => {
  const actions=useEditorStore.getState();
  const second=actions.cutClip('take',1000)!;
  actions.setTrimStart(500); actions.setTrimEnd(1500); actions.setCurrentTime(750);
  actions.setClipPlaybackRate('take',2);
  let state=useEditorStore.getState();
  expect(state.clips[0]).toMatchObject({sourceIn:0,sourceOut:1000,duration:500});
  expect(state.clips[1]).toMatchObject({id:second,startTime:500,sourceIn:1000,sourceOut:2000});
  expect(state.trimStart).toBe(250); expect(state.trimEnd).toBe(1000); expect(state.currentTime).toBe(375);
  expect(state.tracks[0].clips).toEqual(state.clips);
  actions.undo(); state=useEditorStore.getState();
  expect(state.clips[1].startTime).toBe(1000);
  expect(state.trimStart).toBe(500); expect(state.trimEnd).toBe(1500);
  actions.redo(); expect(useEditorStore.getState().clips[1].startTime).toBe(500);
});
it('keeps sequential content through repeated fast/slow edits without changing unrelated tracks', () => {
  const actions=useEditorStore.getState();
  const next=actions.cutClip('take',1000)!;
  const foreign={...useEditorStore.getState().clips[1],id:'other',trackId:'other'};
  useEditorStore.setState({clips:[...useEditorStore.getState().clips,foreign]});
  actions.setClipPlaybackRate('take',0.5);
  expect(useEditorStore.getState().clips.find(c=>c.id===next)?.startTime).toBe(2000);
  expect(useEditorStore.getState().clips.find(c=>c.id==='other')?.startTime).toBe(1000);
  expect(useEditorStore.getState().trimEnd).toBe(3000);
  actions.setClipPlaybackRate('take',1);
  expect(useEditorStore.getState().clips.find(c=>c.id===next)?.startTime).toBe(1000);
  expect(useEditorStore.getState().trimEnd).toBe(2000);
  const history=useEditorStore.getState().history.length;
  actions.setClipPlaybackRate('take',NaN);
  actions.setClipPlaybackRate('take',1);
  expect(useEditorStore.getState().history).toHaveLength(history);
});

it('keeps preview and export on the same source selection after slow motion', () => {
  const actions=useEditorStore.getState();
  actions.cutClip('take',1000);
  actions.setTrimStart(500); actions.setTrimEnd(1500);
  actions.setClipPlaybackRate('take',0.5);
  const state=useEditorStore.getState();
  expect(state.trimStart).toBe(1000);
  expect(state.trimEnd).toBe(2500);
  expect(previewSample(state,state.trimStart).sourceTime).toBe(500);
  expect(previewSample(state,state.trimEnd).sourceTime).toBe(1500);
  expect(exportTimeline(state)).toMatchObject({trim_start:1000,trim_end:2500,
    timeline_clips:[{startTime:0,duration:2000,sourceIn:0,sourceOut:1000},
      {startTime:2000,duration:1000,sourceIn:1000,sourceOut:2000}]});
});

it('applies a clip speed across its track as one atomic undoable edit', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.setClipPlaybackRate(middle,2);
  actions.setTrimStart(250); actions.setTrimEnd(1250);
  const before=useEditorStore.getState();
  const snapshots=before.history.length;
  let notifications=0;
  const unsubscribe=useEditorStore.subscribe(() => notifications++);
  actions.applyClipSpeedToTrack(middle);
  unsubscribe();
  const after=useEditorStore.getState();
  expect(notifications).toBe(1);
  expect(after.history.length).toBe(snapshots+1);
  expect(after.clips.map(c=>[c.id,c.startTime,c.duration,c.playbackRate])).toEqual([
    ['take',0,250,2],[middle,250,500,2],[last,750,250,2],
  ]);
  expect(after.clips.map(c=>[c.sourceIn,c.sourceOut])).toEqual([[0,500],[500,1500],[1500,2000]]);
  expect([after.trimStart,after.trimEnd]).toEqual([125,875]);
  expect(after.tracks[0].clips).toEqual(after.clips);
  actions.undo();
  expect(useEditorStore.getState().clips).toEqual(before.clips);
  expect(useEditorStore.getState().trimEnd).toBe(before.trimEnd);
  actions.redo();
  expect(useEditorStore.getState().clips).toEqual(after.clips);
  actions.applyClipSpeedToTrack(middle);
  expect(useEditorStore.getState().history.length).toBe(snapshots+1);
});

it('batch speed preserves locked rates and other tracks, and rejects locked source tracks', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.setClipPlaybackRate('take',2);
  actions.updateClip(middle,{locked:true});
  const foreign={...useEditorStore.getState().clips[2],id:'foreign',trackId:'other'};
  useEditorStore.setState({clips:[...useEditorStore.getState().clips,foreign]});
  actions.applyClipSpeedToTrack('take');
  let state=useEditorStore.getState();
  expect(state.clips.find(c=>c.id===middle)?.playbackRate).toBe(1);
  expect(state.clips.find(c=>c.id===last)?.playbackRate).toBe(2);
  expect(state.clips.find(c=>c.id==='foreign')).toEqual(foreign);
  actions.undo();
  actions.updateTrack('v',{locked:true});
  const before=useEditorStore.getState();
  actions.applyClipSpeedToTrack('take');
  actions.applyClipSpeedToTrack('missing');
  state=useEditorStore.getState();
  expect(state.clips).toBe(before.clips);
  expect(state.history).toBe(before.history);
});

it('retains high playback speeds through timeline mapping and export', () => {
  const actions=useEditorStore.getState();
  for(const rate of [8,16,24]) {
    actions.setClipPlaybackRate('take',rate);
    const state=useEditorStore.getState();
    expect(state.clips[0].duration).toBeCloseTo(2000/rate);
    expect(previewSample(state,1000/rate).sourceTime).toBeCloseTo(1000);
    expect(exportTimeline(state).timeline_clips?.[0].playbackRate).toBe(rate);
    expect(state.trimEnd).toBeCloseTo(2000/rate);
  }
});

it('merges with next using the selected settings and remaps later clips and source-bound trims', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.setClipPlaybackRate('take',2);
  actions.updateClip('take',{micVolume:0.4,hideCursor:true});
  actions.setTrimStart(125); actions.setTrimEnd(1000); actions.setCurrentTime(750);
  const before=useEditorStore.getState();
  actions.mergeClip('take','next');
  const after=useEditorStore.getState();
  expect(after.clips).toHaveLength(2);
  expect(after.clips[0]).toMatchObject({id:'take',sourceIn:0,sourceOut:1500,startTime:0,duration:750,playbackRate:2,micVolume:0.4,hideCursor:true});
  expect(after.clips[1]).toMatchObject({id:last,startTime:750,duration:500});
  expect([after.trimStart,after.trimEnd,after.currentTime]).toEqual([125,625,500]);
  expect(after.history.length).toBe(before.history.length+1);
  expect(after.tracks[0].clips).toEqual(after.clips);
  actions.undo(); expect(useEditorStore.getState().clips).toEqual(before.clips);
  actions.redo(); expect(useEditorStore.getState().clips).toEqual(after.clips);
});

it('merges with previous keeping the selected identity and restores trimmed source between clips', () => {
  const actions=useEditorStore.getState();
  const second=actions.cutClip('take',1000)!;
  actions.updateClip('take',{sourceOut:500,duration:500});
  actions.updateClip(second,{startTime:500,playbackRate:2,duration:500,micVolume:0.25});
  actions.setTrimEnd(1000); actions.setCurrentTime(750); actions.selectClips(['take']);
  actions.mergeClip(second,'previous');
  const state=useEditorStore.getState();
  expect(state.clips).toHaveLength(1);
  expect(state.clips[0]).toMatchObject({id:second,sourceIn:0,sourceOut:2000,startTime:0,duration:1000,playbackRate:2,micVolume:0.25});
  expect(state.selection.clipIds).toEqual([second]);
  expect(state.currentTime).toBe(750);
  expect(previewSample(state,375).sourceTime).toBe(750);
  expect(exportTimeline(state).timeline_clips?.[0]).toMatchObject({sourceIn:0,sourceOut:2000,playbackRate:2});
});

it('refuses impossible merges and locked edits without adding history', () => {
  const actions=useEditorStore.getState();
  const second=actions.cutClip('take',1000)!;
  for(const updates of [{locked:true},{locked:false,sourceFilePath:'/other.mp4'},
    {sourceFilePath:undefined,sourceIn:0,sourceOut:1000}]) {
    actions.updateClip(second,updates);
    const before=useEditorStore.getState();
    actions.mergeClip('take','next');
    expect(useEditorStore.getState().clips).toBe(before.clips);
    expect(useEditorStore.getState().history).toBe(before.history);
  }
  actions.updateClip(second,{sourceIn:1000,sourceOut:2000});
  actions.updateTrack('v',{locked:true});
  const before=useEditorStore.getState();
  actions.mergeClip('take','next'); actions.mergeClip('take','previous'); actions.mergeClip('missing','next');
  expect(useEditorStore.getState().history).toBe(before.history);
});

it('closes a deleted video interval and preserves remaining source selection through undo', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.setTrimStart(250); actions.setTrimEnd(1750); actions.setCurrentTime(1000);
  const before=useEditorStore.getState();
  actions.deleteClip(middle);
  const after=useEditorStore.getState();
  expect(after.clips.map(c=>[c.id,c.startTime,c.sourceIn,c.sourceOut])).toEqual([
    ['take',0,0,500],[last,500,1500,2000],
  ]);
  expect([after.trimStart,after.trimEnd,after.currentTime]).toEqual([250,750,500]);
  expect(previewSample(after,500).sourceTime).toBe(1500);
  expect(exportTimeline(after).timeline_clips?.[1].startTime).toBe(500);
  expect(after.tracks[0].clips).toEqual(after.clips);
  actions.undo(); expect(useEditorStore.getState().clips).toEqual(before.clips);
  expect(useEditorStore.getState().trimEnd).toBe(1750);
  actions.redo(); expect(useEditorStore.getState().clips).toEqual(after.clips);
});

it('retains the last recording clip, honors locks and resets an entirely deleted trim', () => {
  const actions=useEditorStore.getState();
  actions.deleteClip('take');
  expect(useEditorStore.getState().clips).toHaveLength(1);
  expect(useEditorStore.getState().history).toHaveLength(0);
  const second=actions.cutClip('take',1000)!;
  actions.updateClip(second,{locked:true});
  let before=useEditorStore.getState();
  actions.deleteClip(second);
  expect(useEditorStore.getState().history).toBe(before.history);
  actions.updateClip(second,{locked:false}); actions.updateTrack('v',{locked:true});
  before=useEditorStore.getState(); actions.deleteClip(second);
  expect(useEditorStore.getState().history).toBe(before.history);
  actions.updateTrack('v',{locked:false});
  actions.setTrimStart(1100); actions.setTrimEnd(1800);
  actions.deleteClip(second);
  expect([useEditorStore.getState().trimStart,useEditorStore.getState().trimEnd]).toEqual([0,1000]);
});

it('prunes deleted zoom ranges in source time as part of the same undo step', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  actions.cutClip(middle,1500);
  const block=(id:string,start_time:number,end_time:number)=>({id,start_time,end_time,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:2,is_manual:true});
  for(const zoom of [block('removed',600,1400),block('left-edge',250,1000),block('right-edge',1000,1750),block('boundary',500,1500)]) actions.addZoomBlock(zoom);
  actions.addZoomKeyframe({id:'keyframe',time:700,duration:200,centerX:0.5,centerY:0.5,scale:2,easing:'linear'});
  useEditorStore.setState({selectedBlockId:'removed'});
  const before=useEditorStore.getState();
  actions.deleteClip(middle);
  const after=useEditorStore.getState();
  expect(after.zoomAnalysis?.zoom_blocks.map(b=>b.id)).toEqual(['left-edge','right-edge','boundary']);
  expect(after.zoomKeyframes).toEqual([]);
  expect(after.selectedBlockId).toBeNull(); expect(after.selectedKeyframe).toBeNull();
  expect(after.history.length).toBe(before.history.length+1);
  actions.undo();
  expect(useEditorStore.getState().zoomAnalysis).toEqual(before.zoomAnalysis);
  expect(useEditorStore.getState().zoomKeyframes).toEqual(before.zoomKeyframes);
  actions.redo(); expect(useEditorStore.getState().zoomAnalysis).toEqual(after.zoomAnalysis);
});

it('keeps zooms for retained original-source clips regardless of visibility but ignores foreign source intervals', () => {
  const actions=useEditorStore.getState();
  const second=actions.cutClip('take',1000)!;
  actions.addZoomBlock({id:'orphan',start_time:1200,end_time:1900,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:2,is_manual:true});
  const foreign={...useEditorStore.getState().clips[1],id:'foreign',trackId:'other',sourceFilePath:'/other.mp4'};
  useEditorStore.setState({clips:[...useEditorStore.getState().clips,foreign]});
  actions.updateTrack('v',{visible:false});
  actions.addZoomBlock({id:'kept',start_time:100,end_time:800,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:2,is_manual:true});
  actions.deleteClip(second);
  expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks.map(b=>b.id)).toEqual(['kept']);
});

it('uses endpoint presence when a zoom spans footage whose two ends were removed', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.addZoomBlock({id:'wide',start_time:0,end_time:2000,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:2,is_manual:true});
  actions.deleteClip('take');
  expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks).toHaveLength(1);
  actions.deleteClip(last);
  expect(useEditorStore.getState().clips[0]).toMatchObject({id:middle,sourceIn:500,sourceOut:1500});
  expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks).toHaveLength(0);
  actions.undo(); expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks).toHaveLength(1);
});

it('removes the previous whole clip and keeps the playhead on the same source moment', () => {
  const actions=useEditorStore.getState();
  const middle=actions.cutClip('take',500)!;
  const last=actions.cutClip(middle,1500)!;
  actions.setClipPlaybackRate(middle,2);
  actions.setCurrentTime(1250);
  const before=useEditorStore.getState();
  const source=previewSample(before,before.currentTime).sourceTime;
  actions.removePreviousClip();
  const after=useEditorStore.getState();
  expect(after.clips.map(c=>c.id)).toEqual(['take',last]);
  expect(after.clips[1]).toMatchObject({startTime:500,sourceIn:1500,sourceOut:2000});
  expect(after.currentTime).toBe(750);
  expect(previewSample(after,after.currentTime).sourceTime).toBe(source);
  expect(after.history.length).toBe(before.history.length+1);
  actions.undo(); expect(useEditorStore.getState().clips).toEqual(before.clips);
});

it('does not remove previous footage from gaps, the first clip or a locked predecessor', () => {
  const actions=useEditorStore.getState();
  const second=actions.cutClip('take',1000)!;
  actions.updateClip(second,{startTime:1500});
  for(const time of [500,1250]) {
    actions.setCurrentTime(time);
    const before=useEditorStore.getState();
    actions.removePreviousClip();
    expect(useEditorStore.getState().history).toBe(before.history);
  }
  actions.setCurrentTime(1600); actions.updateClip('take',{locked:true});
  const before=useEditorStore.getState(); actions.removePreviousClip();
  expect(useEditorStore.getState().history).toBe(before.history);
});

it('preserves clip smoothing override through split, export transport and undo', () => {
  const actions=useEditorStore.getState();
  actions.updateClip('take',{disableCursorSmoothing:true});
  const second=actions.cutClip('take',1000)!;
  expect(useEditorStore.getState().clips.every(c=>c.disableCursorSmoothing)).toBe(true);
  actions.updateClip(second,{disableCursorSmoothing:false});
  expect(exportTimeline(useEditorStore.getState()).timeline_clips?.map(c=>c.disableCursorSmoothing)).toEqual([true,false]);
  actions.undo();
  expect(exportTimeline(useEditorStore.getState()).timeline_clips?.map(c=>c.disableCursorSmoothing)).toEqual([true,true]);
});

it('disables camera and magnifier transport without destroying edits, and restores them through undo and reopen', async () => {
  const { exportZoomBlocks } = await import('../../utils/motion/zoomTrajectory');
  const { createProjectDocument, parseProjectDocument } = await import('../../utils/projects/document');
  const actions = useEditorStore.getState();
  actions.addZoomBlock({id:'zoom',start_time:0,end_time:1000,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:2,is_manual:true});
  actions.addZoomBlock({id:'disabled',start_time:1000,end_time:2000,click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,zoom_factor:3,is_manual:true,is_disabled:true});
  const before = useEditorStore.getState();
  const expected = exportZoomBlocks(before.zoomAnalysis, before.visualSettings);
  actions.updateVisualSettings({zoomEffectsEnabled:false});
  const disabled = useEditorStore.getState();
  expect(exportZoomBlocks(disabled.zoomAnalysis, disabled.visualSettings)).toEqual([]);
  expect(disabled.zoomAnalysis).toEqual(before.zoomAnalysis);
  const reopened = parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(disabled)))).state;
  expect(exportZoomBlocks(reopened.zoomAnalysis, reopened.visualSettings)).toEqual([]);
  actions.undo();
  const undone = useEditorStore.getState();
  expect(exportZoomBlocks(undone.zoomAnalysis, undone.visualSettings)).toEqual(expected);
  actions.redo();
  expect(useEditorStore.getState().visualSettings.zoomEffectsEnabled).toBe(false);
  actions.updateVisualSettings({zoomEffectsEnabled:true});
  const enabled = useEditorStore.getState();
  expect(exportZoomBlocks(enabled.zoomAnalysis, enabled.visualSettings)).toEqual(expected);
  expect(expected[1].is_disabled).toBe(true);
});

it('round trips global camera visibility independently of timed layouts and restores it with undo', async () => {
  const { sampleWebcam } = await import('../../utils/camera/webcamTimeline');
  const { createProjectDocument, parseProjectDocument } = await import('../../utils/projects/document');
  const key = {id:'full',time:1000,x:0.5,y:0.5,size:0.2,shape:'circle' as const,visible:true,layout:'fullscreen' as const};
  useEditorStore.setState({webcamKeyframes:[key],hasWebcam:true});
  const actions=useEditorStore.getState();
  actions.updateVisualSettings({webcamEnabled:false});
  const reopened=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(useEditorStore.getState())))).state;
  expect(reopened.webcamKeyframes).toEqual([key]);
  expect(sampleWebcam(reopened.visualSettings,reopened.webcamKeyframes,1500).visible).toBe(false);
  actions.undo();
  const restored=useEditorStore.getState();
  expect(sampleWebcam(restored.visualSettings,restored.webcamKeyframes,1500)).toMatchObject({visible:true,layout:'fullscreen'});
});
it('refreshes the camera lookup after keyframe edits and undo/redo',async()=>{
 const {sampleWebcam}=await import('../../utils/camera/webcamTimeline');
 const actions=useEditorStore.getState();
 actions.addWebcamKeyframe({id:'move',time:1000,x:0.5,y:0.5,size:0.2,shape:'circle',visible:true,layout:'fullscreen'});
 const sample=()=>{const s=useEditorStore.getState();return sampleWebcam(s.visualSettings,s.webcamKeyframes,1500);};
 expect(sample().layout).toBe('fullscreen');
 actions.updateWebcamKeyframe('move',{time:2000});
 expect(sample().layout).toBe('overlay');
 actions.undo();expect(sample().layout).toBe('fullscreen');
 actions.redo();expect(sample().layout).toBe('overlay');
});

it('toggles all video tracks as one saved edit without deleting clips or moving the playhead', async () => {
  const { createProjectDocument, parseProjectDocument } = await import('../../utils/projects/document');
  const actions=useEditorStore.getState();
  const first=useEditorStore.getState().tracks[0];
  useEditorStore.setState({tracks:[first,{...first,id:'hidden',visible:false,clips:[]},{...first,id:'audio',type:'audio',clips:[]}],currentTime:750});
  const before=useEditorStore.getState();
  actions.setVideoTracksVisible(false);
  const hidden=useEditorStore.getState();
  expect(hidden.tracks.map(t=>t.visible)).toEqual([false,false,true]);
  expect(hidden.clips).toEqual(before.clips);
  expect(hidden.currentTime).toBe(750);
  expect(previewSample(hidden,750).sourceTime).toBeNull();
  expect(exportTimeline(hidden).timeline_clips).toEqual([]);
  const restored=parseProjectDocument(JSON.parse(JSON.stringify(createProjectDocument(hidden)))).state;
  expect(restored.tracks.map(t=>t.visible)).toEqual([false,false,true]);
  actions.undo();
  expect(useEditorStore.getState().tracks.map(t=>t.visible)).toEqual([true,false,true]);
  expect(previewSample(useEditorStore.getState(),750).sourceTime).toBe(750);
  actions.redo();
  expect(exportTimeline(useEditorStore.getState()).timeline_clips).toEqual([]);
  actions.setVideoTracksVisible(true);
  expect(exportTimeline(useEditorStore.getState()).timeline_clips).toHaveLength(1);
});

it('updates the playback clock without traversing frozen project data', () => {
  const before = useEditorStore.getState();
  const frozenCheck = vi.spyOn(Object, 'isFrozen');
  before.setCurrentTime(16, 'playback');
  before.setIsPlaying(true);
  before.setCurrentTime(500);
  const checks = frozenCheck.mock.calls.length;
  frozenCheck.mockRestore();
  const after = useEditorStore.getState();
  expect(checks).toBe(0);
  expect(after.currentTime).toBe(500);
  expect(after.playbackRevision).toBe(before.playbackRevision + 2);
  expect(after.clips).toBe(before.clips);
  expect(after.history).toBe(before.history);
});


it.each([null, 'zoom'])('undoes and redoes a targeted split (%s) in one step', (target) => {
  const analysis = { total_clicks: 0, session_duration: 2000,
    config: { enabled: true, zoom_factor: 2, zoom_duration: 2000, min_click_spacing: 500 },
    zoom_blocks: [{ id: 'zoom', start_time: 200, end_time: 1800, zoom_factor: 2,
      click_x: 0.5, click_y: 0.5, center_x: 0.5, center_y: 0.5, is_manual: true }],
  };
  useEditorStore.setState({ zoomAnalysis: analysis });
  const before = useEditorStore.getState();
  splitTimelineAtTime(before, 1000, target);
  const after = useEditorStore.getState();
  expect(after.clips).toHaveLength(target === null ? 2 : 1);
  expect(after.zoomAnalysis?.zoom_blocks).toHaveLength(target === null ? 1 : 2);
  after.undo();
  const undone = useEditorStore.getState();
  expect(undone.clips).toEqual(before.clips);
  expect(undone.zoomAnalysis).toEqual(before.zoomAnalysis);
  expect(undone.canUndo).toBe(false);
  undone.redo();
  const redone = useEditorStore.getState();
  expect(redone.clips).toEqual(after.clips);
  expect(redone.zoomAnalysis).toEqual(after.zoomAnalysis);
});


it.each([99,100,1900,1901])('enforces the zoom split minimum at source time %s without no-op history', (time) => {
  useEditorStore.setState({ zoomAnalysis: { total_clicks: 0, session_duration: 2000,
    config: { enabled: true, zoom_factor: 2, zoom_duration: 2000, min_click_spacing: 500 },
    zoom_blocks: [{ id: 'zoom', start_time: 0, end_time: 2000, zoom_factor: 2,
      click_x: 0.5, click_y: 0.5, center_x: 0.5, center_y: 0.5, is_manual: true }],
  }});
  splitTimelineAtTime(useEditorStore.getState(), time, 'zoom');
  const state = useEditorStore.getState();
  const allowed = time === 100 || time === 1900;
  expect(state.zoomAnalysis?.zoom_blocks).toHaveLength(allowed ? 2 : 1);
  expect(state.canUndo).toBe(allowed);
  if (allowed) expect(state.zoomAnalysis?.zoom_blocks.map(b => b.end_time-b.start_time).sort((a,b)=>a-b)).toEqual([100,1900]);
});


afterEach(() => vi.unstubAllGlobals());
it('keeps the last edited zoom style across undo without remembering position edits', () => {
  let saved: string | null = null;
  vi.stubGlobal('localStorage',{getItem:()=>saved,setItem:(_:string,value:string)=>{saved=value;}});
  useEditorStore.setState({ zoomAnalysis: { total_clicks: 0, session_duration: 2000,
    config: { enabled: true, zoom_factor: 2, zoom_duration: 2000, min_click_spacing: 500 },
    zoom_blocks: [{ id:'zoom', start_time:0, end_time:2000, zoom_factor:2,
      click_x:0.5, click_y:0.5, center_x:0.5, center_y:0.5, is_manual:true }],
  }});
  const actions=useEditorStore.getState();
  actions.updateZoomBlock('zoom',{zoom_factor:3,instant_animation:true});
  expect(readZoomStyle()).toEqual({zoom_factor:3,instant_animation:true,loupe:null});
  actions.undo();
  expect(useEditorStore.getState().zoomAnalysis?.zoom_blocks[0].zoom_factor).toBe(2);
  expect(readZoomStyle().zoom_factor).toBe(3);
  actions.updateZoomBlock('zoom',{center_x:0.7,start_time:100});
  expect(readZoomStyle()).toEqual({zoom_factor:3,instant_animation:true,loupe:null});
});
