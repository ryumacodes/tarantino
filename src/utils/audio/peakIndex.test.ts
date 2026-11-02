import { expect,it } from 'vitest';
import { PeakIndex, peakIndex } from './peakIndex';
it('matches exhaustive range maxima for partial blocks, fractional edges and odd lengths',()=>{
 const peaks=Array.from({length:8193},(_,i)=>((i*1297)%8191)/8191);
 const index=new PeakIndex(peaks);
 for(let i=0;i<2000;i++){
   const start=((i*173)%9000)-100.25,end=start+(i*317)%8200+0.5;
   let expected=0;
   for(let j=Math.max(0,Math.floor(start));j<Math.min(peaks.length,Math.ceil(end));j++)expected=Math.max(expected,peaks[j]);
   expect(index.range(start,end)).toBe(expected);
 }
 expect(index.maximum).toBe(Math.max(...peaks));
 expect(index.range(0,0)).toBe(0);
 expect(index.range(50,10)).toBe(0);
 expect(index.range(NaN,100)).toBe(0);
 expect(new PeakIndex([]).range(0,100)).toBe(0);
});
it('reads at most two partial blocks for a long query and caches immutable source arrays',()=>{
 let reads=0;
 const peaks=new Proxy(Array.from({length:100001},(_,i)=>i===50000?0.9:0.1),{
   get(target,key,receiver){if(typeof key==='string' && /^\d+$/.test(key))reads++;return Reflect.get(target,key,receiver);}
 });
 const index=peakIndex(peaks);reads=0;
 expect(index.range(17,99981)).toBe(0.9);
 expect(reads).toBeLessThanOrEqual(126);
 expect(index.byteLength).toBeLessThan(peaks.length);
 expect(peakIndex(peaks)).toBe(index);
 expect(peakIndex([...peaks])).not.toBe(index);
});
it('benchmarks repeated broad range queries without changing their results',()=>{
 const peaks=Array.from({length:360000},(_,i)=>((i*17)%1001)/1001);
 const built=performance.now(),index=peakIndex(peaks),builtMs=performance.now()-built;
 const ranges=Array.from({length:12000},(_,i)=>{const start=(i*29)%10000;return [start,start+340000] as const;});
 const begin=performance.now();let indexed=0;
 for(const [start,end] of ranges)indexed+=index.range(start,end);
 const indexedMs=performance.now()-begin;
 const scanStart=performance.now();let scanned=0;
 // Use a smaller identical subset for the baseline to keep routine tests bounded.
 for(const [start,end] of ranges.slice(0,100)){let max=0;for(let i=start;i<end;i++)max=Math.max(max,peaks[i]);scanned+=max;}
 const scanMs=performance.now()-scanStart;
 expect(indexed/12000).toBeCloseTo(scanned/100,10);
 expect(scanned/100).toBeCloseTo(1000/1001,10);
 console.log(JSON.stringify({peakIndexBenchmark:{samples:peaks.length,buildMs:builtMs,indexBytes:index.byteLength,indexedQueries:12000,indexedMs,scannedQueries:100,scanMs}}));
});
