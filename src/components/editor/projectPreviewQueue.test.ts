import { expect, it, vi } from 'vitest';
import { createPreviewQueue } from './projectPreviewQueue';
const settle = async () => { for (let i=0;i<8;i++) await Promise.resolve(); };
function setup() {
  const releases = new Map<string, (frame: string) => void>();
  const decode = vi.fn((path: string) => new Promise<string>(resolve => releases.set(path, resolve)));
  return { decode, releases, request: createPreviewQueue(decode) };
}
it('drops invisible queued cards and bounds active decoders while scrolling', async () => {
  const { request, decode, releases } = setup();
  const first = vi.fn();
  const cancelFirst = request('a', first);
  request('b', vi.fn());
  const cancellations = Array.from({length:100}, (_,i) => request(`offscreen-${i}`, vi.fn()));
  cancellations.forEach(cancel => cancel());
  cancelFirst(); cancelFirst();
  const visible = vi.fn(); request('visible', visible);
  await settle(); expect(decode.mock.calls.map(call => call[0])).toEqual(['a','b']);
  releases.get('a')!('frame-a'); await settle();
  expect(first).not.toHaveBeenCalled();
  expect(decode.mock.calls.map(call => call[0])).toEqual(['a','b','visible']);
  releases.get('visible')!('frame-visible'); releases.get('b')!('frame-b'); await settle();
  expect(visible).toHaveBeenCalledWith('frame-visible');
  const cached = vi.fn(); request('a', cached);
  expect(cached).toHaveBeenCalledWith('frame-a'); expect(decode).toHaveBeenCalledTimes(3);
});
it('preserves shared demand, retries cancelled requests, and cancellation is idempotent', async () => {
  const { request, decode, releases } = setup();
  request('a', vi.fn()); request('b', vi.fn());
  const discarded = vi.fn(), retained = vi.fn();
  const cancel = request('shared', discarded); request('shared', retained); cancel();
  const cancelOld = request('retry', vi.fn()); cancelOld();
  const retry = vi.fn(); request('retry', retry); cancelOld();
  await settle(); releases.get('a')!('a'); releases.get('b')!('b'); await settle();
  expect(decode.mock.calls.map(call => call[0])).toEqual(['a','b','shared','retry']);
  releases.get('shared')!('shared'); releases.get('retry')!('retry'); await settle();
  expect(discarded).not.toHaveBeenCalled(); expect(retained).toHaveBeenCalledWith('shared');
  expect(retry).toHaveBeenCalledWith('retry');
});
it('frees slots after failed decoding and does not cache failures', async () => {
  const decode = vi.fn().mockRejectedValueOnce(new Error('missing')).mockResolvedValue('ok');
  const request = createPreviewQueue(decode, 1);
  const failed = vi.fn(), next = vi.fn();
  request('missing', failed); request('next', next); await settle(); await settle();
  expect(failed).toHaveBeenCalledWith(''); expect(next).toHaveBeenCalledWith('ok');
  request('missing', vi.fn()); await settle(); expect(decode).toHaveBeenCalledTimes(3);
});
it('keeps recently reused thumbnails within the bounded cache', async () => {
  const decode = vi.fn(async (path: string) => path);
  const request = createPreviewQueue(decode, 2, 2);
  request('a', vi.fn()); request('b', vi.fn()); await settle();
  request('a', vi.fn()); request('c', vi.fn()); await settle();
  request('a', vi.fn()); request('b', vi.fn()); await settle();
  expect(decode.mock.calls.map(call => call[0])).toEqual(['a','b','c','b']);
});
