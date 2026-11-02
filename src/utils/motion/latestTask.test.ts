import { expect, it, vi } from 'vitest';
import { createLatestTaskQueue } from './latestTask';
const deferred = <T>() => {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes,no) => { resolve=yes;reject=no; });
  return {promise,resolve,reject};
};
const flush = async () => { for(let i=0;i<8;i++)await Promise.resolve(); };
it('runs only the initial and newest calculation during a burst, never concurrently',async()=>{
  const queue=createLatestTaskQueue(),first=deferred<number>(),last=deferred<number>();
  const published:number[]=[],started:number[]=[];
  let active=0,maxActive=0;
  const run=(id:number,promise:Promise<number>)=>async()=>{
    started.push(id);maxActive=Math.max(maxActive,++active);
    try{return await promise;}finally{active--;}
  };
  let cancel=queue.submit(run(0,first.promise),value=>published.push(value),()=>{});
  for(let id=1;id<=100;id++){
    cancel();
    cancel=queue.submit(run(id,last.promise),value=>published.push(value),()=>{});
  }
  expect(started).toEqual([0]);
  first.resolve(0);await flush();
  expect(started).toEqual([0,100]);expect(published).toEqual([]);
  last.resolve(100);await flush();
  expect(published).toEqual([100]);expect(maxActive).toBe(1);
});
it('drops queued work and stale completions when disabled or unmounted',async()=>{
  const queue=createLatestTaskQueue(),first=deferred<number>();
  const publish=vi.fn(),failure=vi.fn(),next=vi.fn(async()=>2);
  const cancelFirst=queue.submit(()=>first.promise,publish,failure);
  const cancelNext=queue.submit(next,publish,failure);
  cancelFirst();cancelNext();first.reject(new Error('obsolete'));await flush();
  expect(next).not.toHaveBeenCalled();expect(publish).not.toHaveBeenCalled();expect(failure).not.toHaveBeenCalled();
  queue.submit(async()=>3,publish,failure);await flush();
  expect(publish).toHaveBeenCalledWith(3);
});
it('reports current errors and advances after rejected or synchronously throwing work',async()=>{
  const queue=createLatestTaskQueue(),first=deferred<number>();
  const publish=vi.fn(),failure=vi.fn();
  queue.submit(()=>first.promise,publish,failure);
  queue.submit(()=>{throw new Error('sync');},publish,failure);
  first.reject(new Error('async'));await flush();
  expect(failure.mock.calls.map(([e])=>e.message)).toEqual(['async','sync']);
  queue.submit(async()=>4,publish,failure);await flush();
  expect(publish).toHaveBeenCalledWith(4);
});
