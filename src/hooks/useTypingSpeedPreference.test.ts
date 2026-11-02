import { afterEach, expect, it, vi } from 'vitest';
const hook=vi.hoisted(() => ({subscribe:undefined as ((notify: () => void) => () => void) | undefined,read:undefined as (() => number) | undefined}));
vi.mock('react', () => ({useSyncExternalStore: (subscribe: (notify: () => void) => () => void, read: () => number) => {
  hook.subscribe=subscribe;hook.read=read;return read();
}}));
afterEach(()=>{vi.unstubAllGlobals();vi.resetModules();});
it('shares rate updates, restores saved values and survives unavailable storage',async()=>{
  let saved='4.2';
  const storage={getItem:()=>saved,setItem:vi.fn((_key:string,value:string)=>{saved=value;})};
  vi.stubGlobal('localStorage',storage);
  vi.stubGlobal('window',{addEventListener:vi.fn(),removeEventListener:vi.fn()});
  const {useTypingSpeedPreference}=await import('./useTypingSpeedPreference');
  const [rate,setRate]=useTypingSpeedPreference();
  expect(rate).toBe(4.2);
  const notify=vi.fn(),unsubscribe=hook.subscribe!(notify);
  setRate(5);
  expect(hook.read!()).toBe(5);expect(saved).toBe('5');expect(notify).toHaveBeenCalledOnce();
  setRate(NaN);setRate(12);expect(hook.read!()).toBe(5);
  storage.setItem.mockImplementation(()=>{throw new Error('storage denied');});
  setRate(6);expect(hook.read!()).toBe(6);
  unsubscribe();expect(window.removeEventListener).toHaveBeenCalled();
});
it('defaults to 3 for corrupt saved rates',async()=>{
  vi.stubGlobal('localStorage',{getItem:()=> 'broken'});
  const {useTypingSpeedPreference}=await import('./useTypingSpeedPreference');
  expect(useTypingSpeedPreference()[0]).toBe(3);
});
