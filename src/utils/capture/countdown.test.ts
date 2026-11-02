import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { readCountdown, saveCountdown, waitForCountdown } from './countdown';
beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] }));
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });
it.each([3, 5, 10] as const)('starts only after the selected %s seconds', async seconds => {
  const tick = vi.fn();
  const beginCapture = vi.fn();
  const ready = waitForCountdown(seconds, new AbortController().signal, tick).then(ok => { if (ok) beginCapture(); });
  expect(tick).toHaveBeenLastCalledWith(seconds);
  await vi.advanceTimersByTimeAsync(seconds * 1000 - 1);
  expect(beginCapture).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  await ready;
  expect(beginCapture).toHaveBeenCalledOnce();
  expect(tick).toHaveBeenLastCalledWith(0);
  expect(vi.getTimerCount()).toBe(0);
});
it('cancellation never starts capture or leaves countdown timers', async () => {
  const controller = new AbortController();
  const beginCapture = vi.fn();
  const ready = waitForCountdown(10, controller.signal, () => {}).then(ok => { if (ok) beginCapture(); });
  await vi.advanceTimersByTimeAsync(2500);
  controller.abort();
  await ready;
  await vi.advanceTimersByTimeAsync(20000);
  expect(beginCapture).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});
it('Off is immediate, but an aborted operation cannot start', async () => {
  const controller = new AbortController();
  const tick = vi.fn();
  expect(await waitForCountdown(0, controller.signal, tick)).toBe(true);
  controller.abort();
  expect(await waitForCountdown(0, controller.signal, tick)).toBe(false);
  expect(tick).not.toHaveBeenCalled();
});
it('uses the elapsed deadline after timer throttling', async () => {
  const tick = vi.fn();
  const ready = waitForCountdown(3, new AbortController().signal, tick);
  vi.spyOn(performance, 'now').mockReturnValue(4000);
  await vi.advanceTimersByTimeAsync(100);
  expect(await ready).toBe(true);
  expect(tick.mock.calls.map(call => call[0])).toEqual([3, 0]);
  vi.restoreAllMocks();
});
it('roundtrips preferences and ignores corrupt or unavailable storage', () => {
  let saved: string | null = null;
  vi.stubGlobal('localStorage', { getItem: () => saved, setItem: (_: string, value: string) => { saved = value; } });
  expect(readCountdown()).toBe(0);
  saveCountdown(5);
  expect(readCountdown()).toBe(5);
  saved = '9';
  expect(readCountdown()).toBe(0);
  vi.stubGlobal('localStorage', { getItem: () => { throw new Error('unavailable'); } });
  expect(readCountdown()).toBe(0);
});
