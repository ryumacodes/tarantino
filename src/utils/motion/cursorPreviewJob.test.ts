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
  await vi.waitFor(() => expect(finish).toBeDefined());
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


it('releases the replacement when restarted work finishes', async () => {
  const end = vi.fn(async () => {});
  const begin = vi.fn().mockResolvedValueOnce('first').mockImplementationOnce(async () => {
    expect(end).not.toHaveBeenCalled();
    return 'second';
  });
  const job = new CursorPreviewJob(begin, end);
  await job.run(async id => {
    expect(id).toBe('first');
    expect(await job.replace()).toBe('second');
    expect(end).toHaveBeenCalledExactlyOnceWith('first');
  });
  expect(end.mock.calls).toEqual([['first'], ['second']]);
});

it('releases a replacement registered after cancellation without returning its id', async () => {
  let registered!: (id: string) => void;
  const begin = vi.fn().mockResolvedValueOnce('first').mockImplementationOnce(() => new Promise<string>(resolve => {registered = resolve;}));
  const end = vi.fn(async () => {});
  const job = new CursorPreviewJob(begin, end);
  await job.run(async () => {
    const replacing = job.replace();
    await vi.waitFor(() => expect(registered).toBeDefined());
    await job.cancel();
    registered('second');
    expect(await replacing).toBeNull();
  });
  expect(end.mock.calls).toEqual([['first'], ['second']]);
});
