import {describe,it,expect} from 'vitest';
import {shortcutAt,shortcutIntervals,type ShortcutEvent} from './intervals';
const down=(timestamp:number,key='c',modifiers:ShortcutEvent['modifiers']=['command']):ShortcutEvent=>
  ({timestamp,key,modifiers,type:'down'});
describe('recorded shortcut source intervals',()=>{
  it('does not invent labels from missing or ordinary typing activity',()=>{
    expect(shortcutIntervals([])).toEqual([]);
    expect(shortcutIntervals([down(100,'a',[]),down(400,'b',[])])).toEqual([]);
    expect(shortcutIntervals([down(100,'ArrowLeft',[])]).map(x=>x.key)).toEqual(['ArrowLeft']);
  });
  it('groups repeated chords through the inclusive two-second boundary',()=>{
    const events=[down(100),{...down(2100),repeat:true},down(4101)];
    const intervals=shortcutIntervals(events);
    expect(intervals.map(x=>[x.firstDown,x.lastDown,x.start,x.end,x.presses]))
      .toEqual([[100,2100,0,3600,2],[4101,4101,4001,5601,1]]);
  });
  it('uses observed modifier holds, a three-second lookback and prior display spacing',()=>{
    const intervals=shortcutIntervals([down(4000),down(4300,'v')],
      [500,1000,1500,3000].map(timestamp=>({timestamp,modifiers:['command']})));
    expect(intervals[0].start).toBe(900);
    expect(intervals[1].start).toBe(4200);
    expect(shortcutIntervals([down(4000)], [{timestamp:3500,modifiers:['command']},
      {timestamp:3800,modifiers:[]}])[0].start).toBe(3900);
  });
  it('tracks release time without allowing repeats to replace the original press',()=>{
    const intervals=shortcutIntervals([down(100),{...down(200),repeat:true},
      {...down(400,'c',[]),type:'up'}]);
    expect(intervals[0]).toMatchObject({firstDown:100,lastDown:200,lastEvent:400,end:1700,presses:2});
  });
  it('uses the latest nonhidden overlapping shortcut and does not resurrect expired ones',()=>{
    const intervals=shortcutIntervals([down(1000),down(1500,'v')]);
    expect(shortcutAt(intervals,1400)?.key).toBe('v');
    expect(shortcutAt(intervals,1400,new Set([1500]))?.key).toBe('c');
    expect(shortcutAt(intervals,3000)?.key).toBe('v');
    expect(shortcutAt(intervals,3001)).toBeNull();
  });
  it('suppresses shifted typing and short unmodified typing pairs',()=>{
    expect(shortcutIntervals([down(100,'A',['shift']),down(500,'b',[])])).toEqual([]);
    expect(shortcutIntervals([down(100,'a',[]),down(999,'b',[])],[],true)).toEqual([]);
    expect(shortcutIntervals([down(100,'a',[]),down(1100,'b',[])],[],true)).toHaveLength(2);
  });
  it('sorts without mutating caller data and canonicalizes modifier order',()=>{
    const input=[down(500,'x',['command','shift']),down(100,'x',['shift','command'])];
    expect(shortcutIntervals(input)[0].presses).toBe(2);
    expect(input[0].timestamp).toBe(500);
    expect(shortcutIntervals([down(NaN),down(-1)])).toEqual([]);
  });
});
