import { describe, expect, it } from 'vitest';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { editSourceRange, newSourceZoomRange, projectSourceRange, zoomRangeLimits, indexZoomRangeLimits } from './sourceRanges';
const clip=(id:string,startTime:number,sourceIn:number,duration:number,playbackRate:number):TimelineClip => ({
  id,name:id,type:'video',trackId:'video',startTime,sourceIn,duration,playbackRate,
  sourceOut:sourceIn+duration*playbackRate,enabled:true,locked:false,
});
const track:TimelineTrack={id:'video',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const state={duration:6000,tracks:[track],clips:[clip('fast',0,4000,1000,2),clip('slow',1500,0,4000,0.5)]};
describe('source ranges on the edited timeline', () => {
  it('projects fast, slow, deleted and repeated source ranges', () => {
    expect(projectSourceRange(state,4500,5500)).toMatchObject([{startTime:250,endTime:750,rate:2}]);
    expect(projectSourceRange(state,500,1500)).toMatchObject([{startTime:2500,endTime:4500,rate:0.5}]);
    expect(projectSourceRange(state,2500,3500)).toEqual([]);
    const repeated={...state,clips:[...state.clips,clip('repeat',6000,4000,2000,1)]};
    const occurrences=projectSourceRange(repeated,4500,5500);
    expect(occurrences).toHaveLength(2);
    expect(occurrences[1]).toMatchObject({startTime:6500,endTime:7500});
    expect(occurrences[0].occurrenceId).not.toBe(occurrences[1].occurrenceId);
  });
  it('does not draw a source range through a covering higher track', () => {
    const upper={...track,id:'upper',order:-1};
    const covered={...state,tracks:[track,upper],clips:[...state.clips,{...clip('cover',250,2500,250,1),trackId:'upper'}]};
    const views=projectSourceRange(covered,4000,6000);
    expect(views.map(v=>[v.startTime,v.endTime])).toEqual([[0,250],[500,1000]]);
  });
  it('translates drags and clipped handles with the occurrence speed', () => {
    const fast=projectSourceRange(state,4500,5500)[0];
    expect(editSourceRange(fast,100,'move',6000)).toEqual({start_time:4700,end_time:5700});
    expect(editSourceRange(fast,100,'start',6000)).toEqual({start_time:4700,end_time:5500});
    expect(editSourceRange(fast,100,'end',6000)).toEqual({start_time:4500,end_time:5700});
    const clipped=projectSourceRange(state,3500,5500)[0];
    expect(editSourceRange(clipped,100,'start',6000)).toEqual({start_time:4200,end_time:5500});
    expect(editSourceRange(clipped,100,'move',6000)).toEqual({start_time:3700,end_time:5700});
    const slow=projectSourceRange(state,500,1500)[0];
    expect(editSourceRange(slow,100,'move',6000)).toEqual({start_time:550,end_time:1550});
  });
  it('bounds edits and creates new zooms in source time, never in a gap', () => {
    const fast=projectSourceRange(state,4500,5500)[0];
    expect(editSourceRange(fast,10000,'move',6000)).toEqual({start_time:5000,end_time:6000});
    expect(editSourceRange(fast,10000,'start',6000)).toEqual({start_time:5300,end_time:5500});
    expect(newSourceZoomRange(state,250,300)).toEqual({start_time:4500,end_time:4800});
    expect(newSourceZoomRange(state,2500)).toEqual({start_time:500,end_time:3500});
    expect(newSourceZoomRange(state,1250)).toBeNull();
    expect(newSourceZoomRange(state,5500)).toBeNull();
  });
});

it('keeps fractional cuts from duplicating a rounded zoom boundary', () => {
  const cut = 3986.8421052631584;
  const split = {...state, duration:9090, clips:[clip('left',0,0,cut,1),clip('right',cut,cut,9090-cut,1)]};
  const views = [[2000,3987],[3987,6000]].flatMap(([start,end]) => projectSourceRange(split,start,end));
  expect(views).toHaveLength(2);
  expect(views.map(v=>[v.startTime,v.endTime])).toEqual([[2000,3987],[3987,6000]]);
  expect(editSourceRange(views[0],100,'end',9090)).toEqual({start_time:2000,end_time:4087});
  expect(projectSourceRange(split,2000,6000)).toMatchObject([{sourceStart:2000,sourceEnd:6000,startTime:2000,endTime:6000}]);
});

it('keeps repetitions, source switches, speed changes and gaps as separate occurrences', () => {
  const first = clip('first',0,0,2000,1);
  for (const second of [
    clip('repeat',2000,0,2000,1),
    {...clip('other-source',2000,2000,2000,1),sourceFilePath:'other.mp4'},
    clip('faster',2000,2000,1000,2),
    clip('gap',2001,2000,2000,1),
    clip('source-cut',2000,2001,1999,1),
  ]) {
    const projected = projectSourceRange({...state,clips:[first,second]},0,4000);
    expect(projected,second.id).toHaveLength(2);
    expect(projected[0].occurrenceId).not.toBe(projected[1].occurrenceId);
  }
});


it('uses source time for insertion and playback time for the resize minimum', () => {
  const timeline = { ...state, clips: [clip('video',0,0,3000,2)] };
  expect(newSourceZoomRange(timeline,2950)).toEqual({start_time:5900,end_time:6000});
  expect(newSourceZoomRange(timeline,2950.5)).toBeNull();
  const view = projectSourceRange(timeline,1000,2000)[0];
  expect(editSourceRange(view,1000,'start',6000)).toEqual({start_time:1800,end_time:2000});
  expect(editSourceRange(view,-1000,'end',6000)).toEqual({start_time:1000,end_time:1200});
  const short = projectSourceRange(timeline,1000,1050)[0];
  expect(editSourceRange(short,1000,'start',6000)).toEqual({start_time:1000,end_time:1050});
});


it('places new zooms in source time and splits an existing zoom instead of overlapping it', () => {
  const timeline = { ...state, clips: [clip('fast',0,0,5000,2)], duration: 10000,
    zoomAnalysis: { total_clicks: 0, session_duration: 10000,
      config: {enabled:true,zoom_factor:2,zoom_duration:3000,min_click_spacing:500},
      zoom_blocks: [{id:'existing',start_time:4000,end_time:6000,zoom_factor:2,
        click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,is_manual:true}] } };
  expect(newSourceZoomRange(timeline,0)).toEqual({start_time:0,end_time:3000});
  expect(newSourceZoomRange(timeline,1500)).toEqual({start_time:3000,end_time:4000});
  expect(newSourceZoomRange(timeline,2000)).toBeNull();
  expect(newSourceZoomRange(timeline,2049.5)).toBeNull();
  expect(newSourceZoomRange(timeline,2050)).toEqual({start_time:4100,end_time:6000,splitBlockId:'existing'});
  expect(newSourceZoomRange(timeline,2950)).toEqual({start_time:5900,end_time:6000,splitBlockId:'existing'});
  expect(newSourceZoomRange(timeline,2950.5)).toBeNull();
  expect(newSourceZoomRange(timeline,3000)).toEqual({start_time:6000,end_time:9000});
  expect(newSourceZoomRange(timeline,4950)).toEqual({start_time:9900,end_time:10000});
});


it('keeps moved and resized zooms between neighbors without changing their durations on move', () => {
  const block = (id:string,start_time:number,end_time:number) => ({id,start_time,end_time,
    zoom_factor:2,is_manual:true,center_x:0.5,center_y:0.5,click_x:0.5,click_y:0.5});
  const blocks=[block('next',4500,5500),block('edited',2000,3000),block('previous',0,1500)];
  const limits=zoomRangeLimits('edited',blocks,6000);
  expect(limits).toEqual({start:1500,end:4500});
  const view={occurrenceId:'source',startTime:2000,endTime:3000,sourceStart:2000,sourceEnd:3000,
    originalStart:2000,originalEnd:3000,rate:1};
  expect(editSourceRange(view,-5000,'move',6000,limits)).toEqual({start_time:1500,end_time:2500});
  expect(editSourceRange(view,5000,'move',6000,limits)).toEqual({start_time:3500,end_time:4500});
  expect(editSourceRange(view,-5000,'start',6000,limits)).toEqual({start_time:1500,end_time:3000});
  expect(editSourceRange(view,5000,'end',6000,limits)).toEqual({start_time:2000,end_time:4500});
  expect(editSourceRange({...view,rate:0.5},5000,'start',6000,limits)).toEqual({start_time:2950,end_time:3000});
  expect(blocks.map(b=>b.id)).toEqual(['next','edited','previous']);
});

it('indexes the same edit constraints for unsorted and equal-start zooms', () => {
  const blocks = [
    {id:'later',start_time:4000,end_time:5000},
    {id:'first',start_time:1000,end_time:1500},
    {id:'tied',start_time:1000,end_time:2000},
    {id:'middle',start_time:2500,end_time:3000},
  ] as import('../../stores/editor/types').ZoomBlock[];
  const original = [...blocks];
  const indexed = indexZoomRangeLimits(blocks, 6000);
  expect(indexed.get('first')).toEqual({start:0,end:1000});
  expect(indexed.get('tied')).toEqual({start:1500,end:2500});
  expect(indexed.get('later')).toEqual({start:3000,end:6000});
  for (const block of blocks) expect(indexed.get(block.id)).toEqual(zoomRangeLimits(block.id,blocks,6000));
  expect(blocks).toEqual(original);
  expect(indexZoomRangeLimits([],6000).size).toBe(0);
});
