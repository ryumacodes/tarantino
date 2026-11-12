import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { observePausedFrameRecovery } from './pausedFrameRecovery';
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
function fixture() {
  let callback: VideoFrameRequestCallback;
  const video = Object.assign(new EventTarget(), {
    paused: true, seeking: false,
    requestVideoFrameCallback: vi.fn((next: VideoFrameRequestCallback) => { callback = next; return 1; }),
    cancelVideoFrameCallback: vi.fn(),
  });
  const options = { request: vi.fn(async () => 'frame'), publish: vi.fn(), dispose: vi.fn(), clear: vi.fn(), error: vi.fn() };
  const recovery = observePausedFrameRecovery(video, options);
  const seek = () => { video.dispatchEvent(new Event('seeking')); video.dispatchEvent(new Event('seeked')); };
  const decoded = () => callback(0, { presentationTime: 0, expectedDisplayTime: 0, width: 100, height: 100, mediaTime: 1, presentedFrames: 1 });
  return { video, options, recovery, seek, decoded };
}
it('recovers a completed paused seek only when no decoded frame arrives', async () => {
  const f = fixture(); f.seek();
  await vi.advanceTimersByTimeAsync(249); expect(f.options.request).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1); expect(f.options.publish).toHaveBeenCalledWith('frame');
  f.decoded(); expect(f.options.clear).toHaveBeenCalled();
  f.seek(); f.decoded();
  await vi.advanceTimersByTimeAsync(250); expect(f.options.request).toHaveBeenCalledTimes(1);
  f.recovery.stop();
});
it('does not recover playback seeks or seeks still in progress', async () => {
  const f = fixture(); f.video.paused = false; f.seek();
  await vi.advanceTimersByTimeAsync(300); expect(f.options.request).not.toHaveBeenCalled();
  f.video.paused = true; f.seek(); f.video.seeking = true;
  await vi.advanceTimersByTimeAsync(300); expect(f.options.request).not.toHaveBeenCalled();
  f.recovery.stop();
});
it.each(['cancel', 'play', 'decoded', 'stop'])('discards an in-flight result after %s', async action => {
  const f = fixture(); let finish!: (value: string) => void;
  f.options.request.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  f.seek(); await vi.advanceTimersByTimeAsync(250);
  if (action === 'cancel') f.recovery.cancel();
  if (action === 'play') { f.video.paused = false; f.video.dispatchEvent(new Event('play')); }
  if (action === 'decoded') f.decoded();
  if (action === 'stop') f.recovery.stop();
  finish('obsolete'); await vi.advanceTimersByTimeAsync(0);
  expect(f.options.publish).not.toHaveBeenCalled(); expect(f.options.dispose).toHaveBeenCalledWith('obsolete');
  f.recovery.stop();
});
it('keeps only the newest waiting recovery while another decode is running', async () => {
  const f = fixture(); let finish!: (value: string) => void;
  f.options.request.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  f.seek(); await vi.advanceTimersByTimeAsync(250);
  f.seek(); await vi.advanceTimersByTimeAsync(250);
  f.seek(); await vi.advanceTimersByTimeAsync(250);
  expect(f.options.request).toHaveBeenCalledTimes(1);
  finish('obsolete'); await vi.advanceTimersByTimeAsync(0);
  expect(f.options.request).toHaveBeenCalledTimes(2);
  expect(f.options.publish.mock.calls).toEqual([['frame']]);
  expect(f.options.dispose).toHaveBeenCalledWith('obsolete'); f.recovery.stop();
});
