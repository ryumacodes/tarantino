import { expect, it, vi } from 'vitest';
import { CursorPreviewJob } from './cursorPreviewJob';

it('releases a job cancelled before registration completes without starting work', async () => {
  let registered!: (id: string) => void;
  const begin = () => new Promise<string>(resolve => { registered = resolve; });
  const end = vi.fn(async () => {});
  const task = vi.fn(async () => {});
  const job = new CursorPreviewJob(begin, end);
  const running = job.run(task);
  await job.cancel();
  registered('old');
  await running;
  expect(task).not.toHaveBeenCalled();
  expect(end).toHaveBeenCalledExactlyOnceWith('old');
});

it('cancels active native work without waiting for its result', async () => {
  let finish!: () => void;
  const task = () => new Promise<void>(resolve => { finish = resolve; });
  const end = vi.fn(async () => {});
  const job = new CursorPreviewJob(async () => 'active', end);
  const running = job.run(task);
  await Promise.resolve();
  await job.cancel();
  expect(end).toHaveBeenCalledExactlyOnceWith('active');
  finish();
  await running;
  expect(end).toHaveBeenCalledTimes(1);
});

it('releases failed work and does not register tasks cancelled while queued', async () => {
  const begin = vi.fn(async () => 'failed');
  const end = vi.fn(async () => {});
  const job = new CursorPreviewJob(begin, end);
  await expect(job.run(async () => { throw new Error('planning failed'); })).rejects.toThrow('planning failed');
  expect(end).toHaveBeenCalledExactlyOnceWith('failed');
  await job.cancel();
  await job.run(async () => {});
  expect(begin).toHaveBeenCalledTimes(1);
});
