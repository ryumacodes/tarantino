import { describe, expect, it, vi } from 'vitest';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { previewSample, timelineDuration } from './previewTimeline';
import { createMediaSynchronizer, type PreviewMedia } from './syncMedia';
const clip = (id: string, startTime: number, sourceIn: number, duration: number, playbackRate: number): TimelineClip => ({
  id, name: id, type: 'video', trackId: 'v', startTime, duration, sourceIn,
  sourceOut: sourceIn + duration * playbackRate, playbackRate, enabled: true, locked: false,
});
const track: TimelineTrack = { id: 'v', name: 'Video', type: 'video', clips: [], height: 60,
  order: 0, visible: true, solo: false, muted: false, locked: false };
const state = { duration: 2000, tracks: [track], clips: [clip('fast',0,1000,500,2), clip('slow',750,0,1000,0.5)] };
describe('edited preview time', () => {
  it('matches the rendered export fixture through reorder, speed and half-open gaps', () => {
    expect(timelineDuration(state)).toBe(1750);
    for (const [time, source] of [[0,1000],[250,1500],[499,1998],[500,null],[749,null],[750,0],[1250,250],[1749,499.5]]) {
      expect(previewSample(state,time!).sourceTime).toBe(source);
    }
  });
  it('scrubs in any order without retaining previous source time or hidden-track selection', () => {
    for (const time of [1250,250,600,1250,0]) {
      expect(previewSample(state,time).sourceTime).toBe(time === 600 ? null : time >= 750 ? (time-750)*0.5 : 1000+time*2);
    }
    expect(previewSample({ ...state, tracks: [{ ...track, visible:false }] },250).sourceTime).toBeNull();
    expect(previewSample(state,250).sourceTime).toBe(1500);
    expect(timelineDuration({ ...state, clips:[clip('slow',0,0,4000,0.5)] })).toBe(4000);
  });
});
function media(): PreviewMedia {
  return { currentTime:0,playbackRate:1,preservesPitch:false,paused:true,seeking:false,readyState:4,muted:false,
    play:vi.fn(async function(this: PreviewMedia) { this.paused=false; }),
    pause:vi.fn(function(this: PreviewMedia) { this.paused=true; }) };
}
describe('media following the edited clock', () => {
  it('seeks across cuts, preserves pitch and pauses/mutes gaps', async () => {
    const video = media(); const sync = createMediaSynchronizer(video,vi.fn());
    sync(previewSample(state,250),true,true);
    expect(video.currentTime).toBe(1.5); expect(video.playbackRate).toBe(2); expect(video.preservesPitch).toBe(true);
    await Promise.resolve();
    sync(previewSample(state,600),true,true);
    expect(video.paused).toBe(true); expect(video.muted).toBe(true);
    sync(previewSample(state,1250),false,true);
    expect(video.currentTime).toBe(0.25); expect(video.playbackRate).toBe(0.5); expect(video.paused).toBe(true);
  });
  it('does not continually seek or restart a playing decoder within its drift tolerance', () => {
    const video=media(); video.paused=false;
    const sync=createMediaSynchronizer(video,vi.fn());
    sync(previewSample(state,250),true);
    video.currentTime=1.52;
    sync(previewSample(state,260),true);
    expect(video.currentTime).toBe(1.52); expect(video.play).not.toHaveBeenCalled();
    sync(previewSample(state,260),false);
    expect(video.paused).toBe(true);
  });
});

it('does not resume media when a pending play resolves after entering a gap', async () => {
  const video=media();
  let finish!: () => void;
  video.play=vi.fn(() => new Promise<void>(resolve => { finish=() => { video.paused=false; resolve(); }; }));
  const sync=createMediaSynchronizer(video,vi.fn());
  sync(previewSample(state,250),true);
  sync(previewSample(state,260),true);
  expect(video.play).toHaveBeenCalledTimes(1);
  sync(previewSample(state,600),true);
  finish(); await Promise.resolve(); await Promise.resolve();
  expect(video.paused).toBe(true);
});
it('finishes a cut seek after an earlier browser seek settles', () => {
  const video=media(); const sync=createMediaSynchronizer(video,vi.fn());
  sync(previewSample(state,0),false);
  video.seeking=true;
  sync(previewSample(state,1250),false);
  expect(video.currentTime).toBe(1);
  video.seeking=false;
  sync(previewSample(state,1250),false);
  expect(video.currentTime).toBe(0.25);
});

it('holds the last source frame at the stopped project endpoint', () => {
  expect(previewSample(state,1750).sourceTime).toBeCloseTo(500,2);
  expect(previewSample(state,1751).sourceTime).toBeNull();
});

it('keeps per-clip cursor visibility in edited time across speed changes and boundaries', () => {
  const hidden = { ...state, clips: state.clips.map(c => ({ ...c, hideCursor: c.id === 'fast' })) };
  for (const t of [0, 250, 499]) expect(previewSample(hidden, t).hideCursor).toBe(true);
  expect(previewSample(hidden, 600).hideCursor).toBe(true);
  for (const t of [750, 1250, 1750]) expect(previewSample(hidden, t).hideCursor).toBe(false);
  expect(previewSample(state, 0).hideCursor).toBe(false);
});

it('selects independent audio levels at cuts and restores unity for older clips', () => {
  const edited = { ...state, clips: state.clips.map(c => ({ ...c,
    micVolume: c.id === 'fast' ? 0 : 0.25, systemVolume: c.id === 'fast' ? 1 : 0.5 })) };
  expect(previewSample(edited, 499)).toMatchObject({ micVolume: 0, systemVolume: 1 });
  expect(previewSample(edited, 750)).toMatchObject({ micVolume: 0.25, systemVolume: 0.5 });
  expect(previewSample(edited, 600).muted).toBe(true);
  expect(previewSample(state, 0)).toMatchObject({ micVolume: 1, systemVolume: 1 });
});

it('keeps fast preview frames advancing when the WebView rejects a rate, then resumes supported playback', async () => {
  const video=media(); const error=vi.fn(); let rate=1;
  Object.defineProperty(video,'playbackRate',{get:()=>rate,set:value=>{
    if(value>16) throw new Error('Unsupported playback rate');
    rate=value;
  }});
  const sync=createMediaSynchronizer(video,error);
  const sample={sourceTime:12000,rate:24,clipId:'fast',muted:false};
  sync(sample,true,true);
  expect(video.currentTime).toBe(12);
  expect(video.paused).toBe(true); expect(video.muted).toBe(true);
  sync({...sample,sourceTime:14400},true,true);
  expect(video.currentTime).toBe(14.4); expect(error).toHaveBeenCalledTimes(1);
  sync({...sample,rate:2},true,true);
  await Promise.resolve();
  expect(video.playbackRate).toBe(2); expect(video.paused).toBe(false);
  expect(video.muted).toBe(false);
});

it('matches a direct layer-selection oracle across overlaps, boundaries, gaps and track-policy edits', () => {
  let seed=17;
  const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
  const tracks=Array.from({length:6},(_,i)=>({...track,id:`track-${i}`,order:i%3,visible:i!==4,solo:false,muted:i===2}));
  const clips=Array.from({length:100},(_,i)=>({...clip(`clip-${i}`,Math.floor(random()*100)*50,Math.floor(random()*1000),50+Math.floor(random()*25)*50,0.5+Math.floor(random()*4)),trackId:tracks[i%6].id,enabled:i%9!==0}));
  for(const currentTracks of [tracks,tracks.map((t,i)=>({...t,solo:i===3})),tracks.map(t=>({...t,order:-t.order,visible:true}))]) {
    const model={clips,tracks:currentTracks,duration:10000};
    const solo=currentTracks.some(t=>t.solo);
    const times=[...clips.flatMap(c=>[c.startTime,c.startTime+c.duration,c.startTime+c.duration-0.01]),...Array.from({length:1000},()=>random()*7000)];
    for(const time of times) {
      const expected=clips.map(c=>({clip:c,track:currentTracks.find(t=>t.id===c.trackId)!}))
        .filter(({clip:c,track:t})=>c.enabled&&t.visible&&(!solo||t.solo))
        .sort((a,b)=>a.track.order-b.track.order)
        .find(({clip:c})=>c.startTime<=time&&c.startTime+c.duration>time);
      // The stopped endpoint intentionally holds its previous decoded frame.
      if(time===timelineDuration(model)) continue;
      const actual=previewSample(model,time);
      expect(actual.clipId).toBe(expected?.clip.id??null);
      if(expected) {
        expect(actual.sourceTime).toBeCloseTo(expected.clip.sourceIn+(time-expected.clip.startTime)*expected.clip.playbackRate);
        expect(actual.muted).toBe(expected.track.muted);
      }
    }
  }
});

it('invalidates the interval index after a timing edit and keeps legacy duration live', () => {
  expect(previewSample(state,250).clipId).toBe('fast');
  const changed={...state,clips:state.clips.map(c=>c.id==='fast'?{...c,startTime:500}:c)};
  expect(previewSample(changed,250).clipId).toBeNull();
  expect(previewSample(changed,500).clipId).toBe('fast');
  expect(previewSample(state,250).clipId).toBe('fast');
  const legacy={clips:[],tracks:[],duration:1000};
  expect(timelineDuration(legacy)).toBe(1000);
  expect(timelineDuration({...legacy,duration:2000})).toBe(2000);
  expect(previewSample({...legacy,duration:2000},1500).sourceTime).toBe(1500);
});

it('honors explicit sub-tolerance seeks without seeking every playback tick', () => {
  const video = media();
  video.paused = false;
  const sync = createMediaSynchronizer(video, vi.fn());
  const sample = { sourceTime: 1000, rate: 1, clipId: 'take', muted: false };
  sync(sample, true, false, 1);
  video.currentTime = 1.01;
  sync({ ...sample, sourceTime: 1020 }, true, false, 1);
  expect(video.currentTime).toBe(1.01);
  sync({ ...sample, sourceTime: 1040 }, true, false, 2);
  expect(video.currentTime).toBe(1.04);
  video.currentTime = 1.05;
  sync({ ...sample, sourceTime: 1060 }, true, false, 2);
  expect(video.currentTime).toBe(1.05);
  sync({ ...sample, sourceTime: 1030 }, true, false, 3);
  expect(video.currentTime).toBe(1.03);
});

it.each(['seeking', 'metadata'] as const)('retains an explicit seek while waiting for %s', waiting => {
  const video = media();
  video.paused = false;
  const sync = createMediaSynchronizer(video, vi.fn());
  const sample = { sourceTime: 1000, rate: 1, clipId: 'take', muted: false };
  sync(sample, true, true, 1);
  if (waiting === 'seeking') video.seeking = true;
  else video.readyState = 0;
  sync({ ...sample, sourceTime: 1020 }, true, true, 2);
  expect(video.currentTime).toBe(1);
  sync({ ...sample, sourceTime: 1040 }, true, true, 3);
  expect(video.currentTime).toBe(1);
  video.seeking = false;
  video.readyState = 4;
  sync({ ...sample, sourceTime: 1040 }, true, true, 3);
  expect(video.currentTime).toBe(1.04);
});
