import { describe, expect, it } from 'vitest';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { shortcutIntervals } from './intervals';
import { shortcutAtOutputTime } from './timeline';

const track: TimelineTrack = { id:'v', name:'Video', type:'video', clips:[], height:60,
  order:0, visible:true, solo:false, muted:false, locked:false };
const clip = (id:string, startTime:number, sourceIn:number, duration:number, playbackRate=1):TimelineClip => ({
  id, name:id, type:'video', trackId:'v', startTime, sourceIn, duration, playbackRate,
  sourceOut:sourceIn+duration*playbackRate, enabled:true, locked:false,
});
const intervals = shortcutIntervals([{timestamp:1100,key:'c',modifiers:['command'],type:'down'}]);
const source = () => ({intervals});
const state = { duration:4000, trimStart:100, trimEnd:3500, tracks:[track],
  clips:[clip('fast',0,1000,500,2),clip('repeat',1000,1000,2000,0.5)] };

describe('shortcut output clock',()=>{
  it('maps trims, repeated source sections and speed edits without changing source identity',()=>{
    for (const time of [0,399,900,1500,2899]) {
      expect(shortcutAtOutputTime(state,time,source)).toBe(intervals[0]);
    }
    for (const time of [-1,400,899,2900,Infinity,NaN]) {
      expect(shortcutAtOutputTime(state,time,source)).toBeNull();
    }
  });
  it('uses track visibility, enabled state, solo and layer priority from video playback',()=>{
    expect(shortcutAtOutputTime({...state,tracks:[{...track,visible:false}]},0,source)).toBeNull();
    expect(shortcutAtOutputTime({...state,clips:state.clips.map(c=>({...c,enabled:false}))},0,source)).toBeNull();
    const top = {...track,id:'top',order:-1};
    const other = {...clip('other',0,0,500),trackId:'top',sourceFilePath:'other.mp4'};
    const resolve = (path:string|undefined) => path ? undefined : source();
    expect(shortcutAtOutputTime({...state,tracks:[track,top],clips:[...state.clips,other]},0,resolve)).toBeNull();
    expect(shortcutAtOutputTime({...state,tracks:[{...track,solo:true},top],clips:[...state.clips,other]},0,resolve)).toBe(intervals[0]);
  });
  it('honors hidden labels and does not fabricate data for old recordings',()=>{
    expect(shortcutAtOutputTime(state,0,()=>undefined)).toBeNull();
    expect(shortcutAtOutputTime(state,0,()=>({intervals,hidden:new Set([1100])}))).toBeNull();
  });
  it('supports legacy unedited recordings and excludes the trim endpoint',()=>{
    const legacy={...state,clips:[],tracks:[],trimStart:1000,trimEnd:2000};
    expect(shortcutAtOutputTime(legacy,0,source)).toBe(intervals[0]);
    expect(shortcutAtOutputTime(legacy,999,source)).toBe(intervals[0]);
    expect(shortcutAtOutputTime(legacy,1000,source)).toBeNull();
  });
});

it('suppresses visible fullscreen camera sections in source time through speed edits and repeats',async()=>{
  const {DEFAULT_VISUAL_SETTINGS}=await import('../../stores/editor/constants');
  const key={x:0.5,y:0.5,size:0.2,shape:'circle' as const,visible:true};
  const camera={...state,hasWebcam:true,visualSettings:{...DEFAULT_VISUAL_SETTINGS,webcamEnabled:true},
    webcamKeyframes:[{...key,id:'full',time:1100,layout:'fullscreen' as const},{...key,id:'overlay',time:1400,layout:'overlay' as const}]};
  for(const time of [0,1100])expect(shortcutAtOutputTime(camera,time,source)).toBeNull();
  for(const time of [150,1700])expect(shortcutAtOutputTime(camera,time,source)).toBe(intervals[0]);
  expect(shortcutAtOutputTime({...camera,hasWebcam:false},0,source)).toBe(intervals[0]);
  expect(shortcutAtOutputTime({...camera,visualSettings:{...camera.visualSettings,webcamEnabled:false}},0,source)).toBe(intervals[0]);
});
