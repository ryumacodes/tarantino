import { afterEach, expect, it, vi } from 'vitest';
import { useEditorStore } from '../../../stores/editor';
import { observeDecodedFrame, subscribePreviewRedraw } from './previewRedrawEvents';

const initial = useEditorStore.getState();
afterEach(() => useEditorStore.setState(initial, true));

it('redraws paused seeks and edits without adding work on playback ticks', () => {
  useEditorStore.setState({ isPlaying: true });
  const invalidate = vi.fn();
  const stop = subscribePreviewRedraw(invalidate);
  for (let frame = 0; frame < 120; frame++) useEditorStore.setState({ currentTime: frame * 1000 / 60 });
  expect(invalidate).not.toHaveBeenCalled();
  useEditorStore.setState({ isPlaying: false });
  useEditorStore.setState({ currentTime: 300 });
  useEditorStore.setState({ visualSettings: { ...initial.visualSettings, cursorScale: 2 } });
  expect(invalidate.mock.calls).toEqual([[2], [2], [2]]);
  stop();
  useEditorStore.setState({ currentTime: 500 });
  expect(invalidate).toHaveBeenCalledTimes(3);
});

it('refreshes asynchronously decoded frames after the paused seek and removes old listeners', () => {
  const video = new EventTarget();
  const refresh = vi.fn();
  const stop = observeDecodedFrame(video, refresh);
  video.dispatchEvent(new Event('loadeddata'));
  video.dispatchEvent(new Event('seeked'));
  video.dispatchEvent(new Event('timeupdate'));
  expect(refresh).toHaveBeenCalledTimes(2);
  stop();
  video.dispatchEvent(new Event('seeked'));
  expect(refresh).toHaveBeenCalledTimes(2);
});

it('redraws a paused frame delivered after seeked without adding redraws during playback', () => {
  let nextId = 0;
  const callbacks = new Map<number, () => void>();
  const video = Object.assign(new EventTarget(), {
    paused: true,
    requestVideoFrameCallback: (callback: VideoFrameRequestCallback) => {
      callbacks.set(++nextId, () => callback(0, {
        presentationTime: 0, expectedDisplayTime: 0, width: 640, height: 480,
        mediaTime: 0, presentedFrames: nextId,
      }));
      return nextId;
    },
    cancelVideoFrameCallback: (id: number) => { callbacks.delete(id); },
  });
  const presentFrame = () => {
    const pending = [...callbacks.values()];
    callbacks.clear();
    pending.forEach(callback => callback());
  };
  const refresh = vi.fn();
  const stop = observeDecodedFrame(video, refresh);
  video.dispatchEvent(new Event('seeked'));
  expect(refresh).toHaveBeenCalledTimes(1);
  presentFrame();
  expect(refresh).toHaveBeenCalledTimes(2);
  video.paused = false;
  for (let i = 0; i < 120; i++) presentFrame();
  expect(refresh).toHaveBeenCalledTimes(2);
  video.paused = true;
  presentFrame();
  expect(refresh).toHaveBeenCalledTimes(3);
  stop();
  expect(callbacks.size).toBe(0);
  presentFrame();
  video.dispatchEvent(new Event('seeked'));
  expect(refresh).toHaveBeenCalledTimes(3);
});
