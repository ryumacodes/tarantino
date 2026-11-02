import { describe, expect, it } from 'vitest';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
import { editSourceRange, newSourceZoomRange, projectSourceRange } from './sourceRanges';
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
    expect(editSourceRange(fast,10000,'start',6000)).toEqual({start_time:4999,end_time:5500});
    expect(newSourceZoomRange(state,250,300)).toEqual({start_time:4500,end_time:5100});
    expect(newSourceZoomRange(state,2500)).toEqual({start_time:500,end_time:1500});
    expect(newSourceZoomRange(state,1250)).toBeNull();
    expect(newSourceZoomRange(state,5500)).toBeNull();
  });
});
