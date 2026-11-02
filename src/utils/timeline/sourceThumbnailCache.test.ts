import { expect,it,vi } from 'vitest';
import { SourceThumbnailCache, type SourceThumbnail } from './sourceThumbnailCache';
it('deduplicates concurrent source requests and preserves fractional seek times',async()=>{
  let resolve!:(value:SourceThumbnail[])=>void;
  const loader=vi.fn(()=>new Promise<SourceThumbnail[]>(done=>{resolve=done;}));
  const cache=new SourceThumbnailCache(loader);
  const a=cache.ensure([250.125,250.125]), b=cache.ensure([250.125]);
  await Promise.resolve();
  expect(loader).toHaveBeenCalledTimes(1);
  resolve([{source_time_ms:250.125,image:'frame'}]);
  await Promise.all([a,b]);
  expect(cache.get(250.125)).toBe('frame');
  await cache.ensure([250.125]);
  expect(loader).toHaveBeenCalledTimes(1);
});
it('bounds cache size and avoids repeatedly retrying a failed source sample',async()=>{
  const loader=vi.fn(async(times:number[])=>times.map(time=>({source_time_ms:time,image:String(time)})));
  const cache=new SourceThumbnailCache(loader,2);
  await cache.ensure([1,2]); await cache.ensure([3]);
  expect(cache.get(1)).toBeUndefined(); expect(cache.get(3)).toBe('3');
  const failed=vi.fn(async()=>{throw new Error('decoder unavailable');});
  const unavailable=new SourceThumbnailCache(failed);
  await unavailable.ensure([1]); await unavailable.ensure([1]);
  expect(failed).toHaveBeenCalledTimes(1);
});
