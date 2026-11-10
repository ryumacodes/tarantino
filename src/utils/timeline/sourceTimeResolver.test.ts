import { expect, it } from 'vitest';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { sourceTimeResolver } from './sourceTimeResolver';
import { editSourceRange, projectSourceRange, zoomResizeAvailability } from './sourceRanges';

const clip=(id:string,startTime:number,sourceIn:number,sourceOut:number,playbackRate:number):TimelineClip => ({
  id,name:id,type:'video',trackId:'video',startTime,sourceIn,sourceOut,playbackRate,
  duration:(sourceOut-sourceIn)/playbackRate,enabled:true,locked:false,
});
const track:TimelineTrack={id:'video',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const state={duration:6000,tracks:[track],clips:[clip('first',0,0,1000,1),
  clip('fast',1000,2000,4000,2),clip('slow',2000,4000,6000,0.5)]};

it('maps cuts, speed boundaries and endpoints without the preview endpoint inset', () => {
  const resolver=sourceTimeResolver(state,500)!;
  expect([500,1000,1500,2000,3000,4000,6000].map(resolver.toPlayback)).toEqual([500,1000,1000,1000,1500,2000,6000]);
  expect([500,1000,1001,1500,2000,2100,6000,7000].map(resolver.toSource)).toEqual([500,1000,2002,3000,4000,4050,6000,6000]);
});

it('moves a zoom by playback duration across deleted source and different speeds', () => {
  const views=projectSourceRange(state,500,3000);
  const resolver=sourceTimeResolver(state,views[0].startTime)!;
  const moved=editSourceRange(views[0],600,'move',6000,undefined,resolver);
  expect(moved).toEqual({start_time:2200,end_time:4050});
  expect(resolver.toPlayback(moved.end_time)-resolver.toPlayback(moved.start_time)).toBe(1000);
  expect(editSourceRange(views[1],700,'end',6000,undefined,resolver)).toEqual({start_time:500,end_time:4100});
  expect(editSourceRange(views[0],5000,'start',6000,undefined,resolver)).toEqual({start_time:2800,end_time:3000});
  expect(editSourceRange(views[0],5000,'move',6000,{start:0,end:4500},resolver)).toEqual({start_time:4000,end_time:4500});
});

it('keeps the selected repeated occurrence and does not traverse an output gap', () => {
  const repeated={...state,clips:[clip('one',0,0,1000,1),clip('two',1000,0,1000,1),clip('gap',2500,1000,2000,1)]};
  const first=sourceTimeResolver(repeated,200)!;
  const second=sourceTimeResolver(repeated,1200)!;
  expect(first.toPlayback(500)).toBe(500);
  expect(second.toPlayback(500)).toBe(1500);
  expect(second.toSource(2400)).toBe(1000);
  expect(sourceTimeResolver(repeated,2200)).toBeNull();
});


it('offers resizing only when the endpoint has room to expand or shrink on the playback clock', () => {
  const fast = {...state, clips:[clip('fast',0,0,6000,2)]};
  const resolver = sourceTimeResolver(fast,500)!;
  for (const duration of [50,100,101,500]) {
    const view = projectSourceRange(fast,1000,1000+duration*2)[0];
    for (const left of [0,1]) for (const right of [0,1]) {
      expect(zoomResizeAvailability(view, {start:1000-left,end:view.originalEnd+right}, resolver))
        .toEqual({start:left>0 || duration>100,end:right>0 || duration>100});
    }
  }
  const views = projectSourceRange(state,500,3000);
  const mixed = sourceTimeResolver(state,500)!;
  expect(zoomResizeAvailability(views[0],{start:500,end:3000},mixed)).toEqual({start:true,end:false});
  expect(zoomResizeAvailability(views[1],{start:500,end:3000},mixed)).toEqual({start:false,end:true});
});

it('keeps fractional adjacent clips connected despite source rounding', () => {
  const first = clip('first',1000.1,0,1000.2,1);
  const boundary = first.startTime + first.duration;
  const next = clip('next',boundary,1000.2,2000.2,2);
  const edited = {...state,clips:[first,next]};
  const resolver = sourceTimeResolver(edited,1500)!;
  expect(resolver.toPlayback(1400.2)).toBeCloseTo(boundary+200,9);
  expect(resolver.toSource(boundary+200)).toBeCloseTo(1400.2,9);
  const view = projectSourceRange(edited,500,1500)[0];
  const moved = editSourceRange(view,100,'move',6000,undefined,resolver);
  expect(moved).toEqual({start_time:600,end_time:1700});
  const repeated = {...edited,clips:[first,{...next,sourceIn:999.2}]};
  expect(sourceTimeResolver(repeated,1500)!.toPlayback(1400.2)).toBeCloseTo(boundary,9);
});
