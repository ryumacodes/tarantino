import {expect,it} from 'vitest';
import {shortcutIntervals} from './intervals';
import {keycapInstances,keycapTarget} from './keycapInstances';
const intervals=shortcutIntervals([1000,1600,4000].map((timestamp,index)=>({timestamp,key:index===0?'c':'v',modifiers:['command'] as const,type:'down' as const})));
it('keeps common keycaps alive across overlaps but creates new instances after gaps',()=>{
  const instances=keycapInstances(intervals);
  expect(instances.map(i=>[i.label,i.segments.length,i.appearTime,i.end]))
    .toEqual([['⌘',2,900,3100],['C',1,960,1600],['V',1,1560,3100],['⌘',1,3900,5500],['V',1,3960,5500]]);
  const command=instances[0];
  expect(keycapTarget(command,1500).segment.interval.firstDown).toBe(1600);
  expect(keycapTarget(command,1500).opacity).toBe(1);
  const c=instances[1];
  expect(keycapTarget(c,1500)).toMatchObject({opacity:1,travel:0});
  expect(keycapTarget(c,1550)).toMatchObject({opacity:0.5,travel:-0.5});
  expect(keycapTarget(c,1600)).toMatchObject({opacity:0,travel:-1});
});
it('staggered entrance and fullscreen suppression leave travel targets independent',()=>{
  const key=keycapInstances(intervals)[1];
  expect(keycapTarget(key,959)).toMatchObject({opacity:0,travel:-1});
  expect(keycapTarget(key,1010)).toMatchObject({opacity:0.5,travel:-0.5});
  expect(keycapTarget(key,1010,true)).toMatchObject({opacity:0,travel:-0.5});
  expect(keycapTarget(key,1060)).toMatchObject({opacity:1,travel:0});
});
it('hiding a chord removes its handoff and does not mutate source intervals',()=>{
  const before=JSON.stringify(intervals);
  const instances=keycapInstances(intervals,new Set([1600]));
  expect(instances[1].handoffStart).toBeUndefined();
  expect(instances[1].end).toBe(2500);
  expect(JSON.stringify(intervals)).toBe(before);
});
