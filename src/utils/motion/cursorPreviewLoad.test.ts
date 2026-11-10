import { describe, expect, it } from 'vitest';
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
    await loadCursorPreview(60000, () => 0, async until => {
      calls.push(until);
      return 'obsolete';
    }, value => published.push(value), () => true);
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
