import {describe,it,expect} from 'vitest';
import {cursorTrailSamples} from './cursorTrail';
const frames=Array.from({length:240},(_,i)=>[i/240,0.5,1]);
describe('cursor trail simulation clock',()=>{
  it('gives the same history after any render cadence or seek',()=>{
    const expected=cursorTrailSamples(frames,500,0,15);
    for(const hz of [24,60,120,144]) {
      for(let t=0;t<500;t+=1000/hz)cursorTrailSamples(frames,t,0,15);
      expect(cursorTrailSamples(frames,500,0,15)).toEqual(expected);
    }
    expect(expected).toHaveLength(15);
    expect(expected[expected.length-1]?.frame[0]).toBe(60/240);
    expect(cursorTrailSamples(frames,500,0,15)).toEqual(expected);
  });
  it('resets cadence at the clip boundary and caps shader history',()=>{
    expect(cursorTrailSamples(frames,500,500,15)).toHaveLength(1);
    expect(cursorTrailSamples(frames,499,500,15)).toEqual([]);
    expect(cursorTrailSamples(frames,1000,0,500)).toHaveLength(30);
    expect(cursorTrailSamples(frames,500,0,0)).toEqual([]);
  });
  it('selects the entering sample for fractional cut times',()=>{
    const sample=cursorTrailSamples(frames,508.3334,500.1,15,500.1);
    expect(sample).toHaveLength(1);
    expect(sample[0].frame).toBe(frames[61]);
  });
  it('does not fabricate points before cursor entry',()=>{
    const delayed=frames.map((frame,i)=>[...frame.slice(0,2),i<30?0:1]);
    expect(cursorTrailSamples(delayed,250,0,15,250)).toHaveLength(1);
    const idle=frames.map(frame=>[frame[0],frame[1],0]);
    expect(cursorTrailSamples(idle,500,0,15,0)).toHaveLength(15);
  });
});
