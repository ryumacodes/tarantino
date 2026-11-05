import { expect, it, vi } from 'vitest';
import { LatestTaskQueue } from './latestTaskQueue';

it('runs only the newest queued change after the active calculation finishes', async () => {
  const queue = new LatestTaskQueue<number>();
  let finish!: (value: number) => void;
  const started: number[] = [];
  const first = queue.run(() => {
    started.push(0);
    return new Promise<number>(resolve => { finish = resolve; });
  });
  const changes = Array.from({ length: 20 }, (_, index) => queue.run(async () => {
    started.push(index + 1);
    return index + 1;
  }));
  expect(started).toEqual([0]);
  finish(0);
  expect(await first).toBe(0);
  expect(await Promise.all(changes)).toEqual([...Array(19).fill(undefined), 20]);
  expect(started).toEqual([0, 20]);
  expect(await queue.run(async () => 21)).toBe(21);
});

it('clears queued work on cleanup without interrupting the active calculation', async () => {
  const queue = new LatestTaskQueue<string>();
  let finish!: (value: string) => void;
  const active = queue.run(() => new Promise<string>(resolve => { finish = resolve; }));
  const unused = vi.fn(async () => 'obsolete');
  const pending = queue.run(unused);
  queue.clearPending();
  expect(await pending).toBeUndefined();
  finish('active');
  expect(await active).toBe('active');
  expect(unused).not.toHaveBeenCalled();
  expect(await queue.run(async () => 'new project')).toBe('new project');
});

it('continues with the latest request after an active calculation fails', async () => {
  const queue = new LatestTaskQueue<string>();
  let fail!: (error: Error) => void;
  const active = queue.run(() => new Promise<string>((_, reject) => { fail = reject; }));
  const rejected = expect(active).rejects.toThrow('decode failed');
  const next = queue.run(async () => 'recovered');
  fail(new Error('decode failed'));
  await rejected;
  expect(await next).toBe('recovered');
});
