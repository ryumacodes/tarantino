import {expect,it} from 'vitest';
import {typingRanges} from './typingRanges';
const keys=(times:number[])=>times.map(timestamp=>({timestamp}));
it('groups sustained activity with strict gap and duration boundaries',()=>{
  expect(typingRanges(keys([0,300,600,900,1200,1500,2000,2300,2600,2900,3200,3500])))
    .toEqual([{start:0,end:1500},{start:2000,end:3500}]);
  expect(typingRanges(keys([0,300,600,900,1200,1499]))).toEqual([]);
  expect(typingRanges(keys([0,500,1000,1500]))).toEqual([]);
});
it('sorts input, ignores invalid timestamps and supports press/release sidecars',()=>{
  const input=keys([1500,0,300,600,900,1200]);
  expect(typingRanges([...input,null,{timestamp:-1},{timestamp:'2000'},{timestamp:NaN}]))
    .toEqual([{start:0,end:1500}]);
  expect(input[0].timestamp).toBe(1500);
  expect(typingRanges(undefined)).toEqual([]);
});
