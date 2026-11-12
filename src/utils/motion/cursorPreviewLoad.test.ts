import { describe, expect, it, vi } from 'vitest';
import { loadCursorPreview } from './cursorPreviewLoad';

describe('cursor preview loading', () => {
  it('publishes the opening before the full trajectory finishes', async () => {
    const calls: (number | undefined)[] = [];
    const published: [string, boolean][] = [];
    let finish!: (value: string) => void;
    const full = new Promise<string>(resolve => { finish = resolve; });
    const loading = loadCursorPreview(60000, () => 0, async until => {
      calls.push(until);
      return until ? 'opening' : full;
    }, (value, complete) => published.push([value, complete]), () => false);
    await Promise.resolve();
    expect(published).toEqual([['opening', false]]);
    expect(calls).toEqual([5000, undefined]);
    finish('full');
    await loading;
    expect(published).toEqual([['opening', false], ['full', true]]);
  });

  it('does not start full preparation after an obsolete opening finishes', async () => {
    const calls: (number | undefined)[] = [];
    const published: string[] = [];
    let cancelled = false;
    await loadCursorPreview(60000, () => 0, async until => {
      calls.push(until);
      cancelled = true;
      return 'obsolete';
    }, value => published.push(value), () => cancelled);
    expect(calls).toEqual([5000]);
    expect(published).toEqual([]);
  });

  it('loads short recordings only once', async () => {
    const calls: (number | undefined)[] = [];
    const published: [string, boolean][] = [];
    await loadCursorPreview(5000, () => 0, async until => {
      calls.push(until);
      return 'short';
    }, (value, complete) => published.push([value, complete]), () => false);
    expect(calls).toEqual([undefined]);
    expect(published).toEqual([['short', true]]);
  });

  it.each([
    [60000, 32000.25, [37001, undefined]],
    [60000, 55000, [undefined]],
    [60000, 60000, [undefined]],
    [60000, -200, [5000, undefined]],
    [2000, 500, [undefined]],
  ])('covers the playhead at %s ms / %s ms without duplicating a complete request', async (duration, time, expected) => {
    const calls: (number | undefined)[] = [];
    await loadCursorPreview(duration, () => time, async until => {
      calls.push(until);
      return 'frames';
    }, () => {}, () => false);
    expect(calls).toEqual(expected);
  });

  it.each([
    [32000, [5000, 37000, undefined]],
    [5000, [5000, 10000, undefined]],
    [2000, [5000, undefined]],
    [58000, [5000, undefined]],
  ])('prioritizes a playhead moved to %s while the opening is loading', async (destination, expected) => {
    let time = 0;
    const calls: (number | undefined)[] = [];
    const published: [string, boolean][] = [];
    await loadCursorPreview(60000, () => time, async until => {
      calls.push(until);
      time = destination;
      return String(until);
    }, (value, complete) => published.push([value, complete]), () => false);
    expect(calls).toEqual(expected);
    expect(published).toEqual(expected.map(until => [String(until), until === undefined]));
  });

  it('stops after cancellation during a replacement prefix', async () => {
    let time = 0;
    let cancelled = false;
    const calls: (number | undefined)[] = [];
    const published: string[] = [];
    await loadCursorPreview(60000, () => time, async until => {
      calls.push(until);
      if (calls.length === 2) cancelled = true;
      time = 32000;
      return String(until);
    }, value => published.push(value), () => cancelled);
    expect(calls).toEqual([5000, 37000]);
    expect(published).toEqual(['5000']);
  });
});


it('replaces full preparation with a prefix at the latest seek position', async () => {
  let time = 0;
  let notify = () => {};
  let rejectFull!: (error: Error) => void;
  const obsolete = new Promise<string>((_, reject) => { rejectFull = reject; });
  const calls: (number | undefined)[] = [];
  const published: [string, boolean][] = [];
  const unsubscribe = vi.fn();
  const restart = vi.fn(async () => { rejectFull(new Error('cancelled')); });
  const loading = loadCursorPreview(60000, () => time, async until => {
    calls.push(until);
    if (calls.length === 2) return obsolete;
    return String(until);
  }, (value, complete) => published.push([value, complete]), () => false, {
    subscribe: listener => { notify = listener; return unsubscribe; }, restart,
  });
  await vi.waitFor(() => expect(calls).toEqual([5000, undefined]));
  time = 32000;
  notify();
  time = 43000;
  notify();
  await loading;
  expect(restart).toHaveBeenCalledTimes(1);
  expect(calls).toEqual([5000, undefined, 48000, undefined]);
  expect(published).toEqual([['5000', false], ['48000', false], ['undefined', true]]);
  expect(unsubscribe).toHaveBeenCalledTimes(2);
});

it.each([2000, 58000])('keeps full preparation for a covered or near-end seek to %s', async time => {
  let current = 0;
  let notify = () => {};
  let finish!: (value: string) => void;
  const full = new Promise<string>(resolve => { finish = resolve; });
  const restart = vi.fn(async () => {});
  const loading = loadCursorPreview(60000, () => current, async until => until ? 'prefix' : full,
    () => {}, () => false, {subscribe: listener => {notify = listener; return () => {};}, restart});
  await Promise.resolve();
  current = time;
  notify();
  finish('full');
  await loading;
  expect(restart).not.toHaveBeenCalled();
});

it('does not publish an obsolete full result or restart work after cancellation', async () => {
  let time = 0;
  let cancelled = false;
  let notify = () => {};
  let finish!: (value: string) => void;
  const full = new Promise<string>(resolve => { finish = resolve; });
  const compute = vi.fn(async (until?: number) => until ? 'prefix' : full);
  const publish = vi.fn();
  const loading = loadCursorPreview(60000, () => time, compute, publish, () => cancelled, {
    subscribe: listener => {notify = listener; return () => {};},
    restart: async () => {cancelled = true; finish('obsolete');},
  });
  await vi.waitFor(() => expect(compute).toHaveBeenCalledTimes(2));
  time = 20000;
  notify();
  await loading;
  expect(compute).toHaveBeenCalledTimes(2);
  expect(publish).toHaveBeenCalledExactlyOnceWith('prefix', false);
});


it('prioritizes a backward seek when full preparation started near the end', async () => {
  let time = 58000;
  let notify = () => {};
  let rejectFull!: (error: Error) => void;
  const obsolete = new Promise<string>((_, reject) => {rejectFull = reject;});
  const calls: (number | undefined)[] = [];
  const loading = loadCursorPreview(60000, () => time, async until => {
    calls.push(until);
    return calls.length === 1 ? obsolete : 'current';
  }, () => {}, () => false, {
    subscribe: listener => {notify = listener; return () => {};},
    restart: async () => {rejectFull(new Error('cancelled'));},
  });
  time = 20000;
  notify();
  await loading;
  expect(calls).toEqual([undefined, 25000, undefined]);
});

it('reports replacement registration failure after stopping obsolete work', async () => {
  let time = 0;
  let notify = () => {};
  let rejectFull!: (error: Error) => void;
  const full = new Promise<string>((_, reject) => {rejectFull = reject;});
  const loading = loadCursorPreview(60000, () => time, async until => until ? 'prefix' : full,
    () => {}, () => false, {
      subscribe: listener => {notify = listener; return () => {};},
      restart: async () => {rejectFull(new Error('cancelled')); throw new Error('registration failed');},
    });
  const rejected = expect(loading).rejects.toThrow('registration failed');
  await Promise.resolve();
  time = 20000;
  notify();
  await rejected;
});
