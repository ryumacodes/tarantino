import {expect,it} from 'vitest';
import {shortcutsFromSidecar} from './sidecar';
import {typingRanges} from '../timeline/typingRanges';
const sample=(timestamp:number,key:string|null='é',is_release=false)=>({timestamp,is_release,
  is_typing:false,shortcut:{key,modifiers:['command'],repeat:false}});
it('loads native label payloads, modifier observations and releases without inventing legacy labels',()=>{
  const input=[{timestamp:110,is_typing:true},sample(100,null),sample(500),sample(800,'é',true)];
  expect(shortcutsFromSidecar(input)).toEqual([{key:'é',modifiers:['command'],firstDown:500,lastDown:500,
    lastEvent:800,start:0,end:2000,presses:1}]);
  expect(shortcutsFromSidecar([{timestamp:100,is_typing:true}])).toEqual([]);
  expect(shortcutsFromSidecar(null)).toEqual([]);
});
it('rejects malformed timestamps, modifiers and labels',()=>{
  expect(shortcutsFromSidecar([sample(-1),sample(NaN),sample(Infinity),sample(100,'\u0001'),
    sample(200,'x'.repeat(33)),{...sample(300),shortcut:{key:'x',modifiers:['invalid']}}])).toEqual([]);
});
it('does not count added shortcut metadata as sustained typing activity',()=>{
  const data=[0,300,600,900,1200,1500].map(time=>sample(time));
  expect(typingRanges(data)).toEqual([]);
  expect(typingRanges([...data,...[0,300,600,900,1200,1500].map(timestamp=>({timestamp,is_typing:true}))]))
    .toEqual([{start:0,end:1500}]);
});
it('retains shifted named keys and Caps Lock transitions through sidecar loading',()=>{
  const records=[
    {timestamp:100,is_release:true,shortcut:{key:null,modifiers:['shift']}},
    {timestamp:500,is_release:false,shortcut:{key:'Tab',modifiers:['shift']}},
    {timestamp:650,is_release:true,shortcut:{key:'Tab',modifiers:['shift']}},
    {timestamp:2500,is_release:false,shortcut:{key:'CapsLock',modifiers:['capsLock']}},
    {timestamp:2501,is_release:true,shortcut:{key:null,modifiers:['capsLock']}},
    {timestamp:5000,is_release:false,shortcut:{key:'CapsLock',modifiers:[]}},
  ];
  const intervals=shortcutsFromSidecar(records);
  expect(intervals.map(i=>[i.key,i.modifiers,i.lastEvent])).toEqual([
    ['Tab',['shift'],650],['CapsLock',['capsLock'],2500],['CapsLock',[],5000],
  ]);
  expect(intervals[0].start).toBe(0);
});
