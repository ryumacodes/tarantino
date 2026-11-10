import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useEditorStore } from '../../../stores/editor';
import { createMediaSynchronizer } from '../../../utils/timeline/syncMedia';
import { startPreviewMediaSync } from './previewMediaLoop';

const initial = useEditorStore.getState();
let nextId = 0;
const frames = new Map<number, FrameRequestCallback>();
beforeEach(() => {
  useEditorStore.setState({ isPlaying: false });
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.set(++nextId, callback);
    return nextId;
  });
  vi.stubGlobal('cancelAnimationFrame', (id: number) => { frames.delete(id); });
});
afterEach(() => {
  useEditorStore.setState(initial, true);
  frames.clear();
  vi.unstubAllGlobals();
});
function nextFrame() {
  const pending = [...frames.values()];
  frames.clear();
  pending.forEach(callback => callback(0));
}

it('stops polling while paused and keeps exactly one loop during playback', () => {
  const sync = vi.fn();
  const video = new EventTarget();
  const stop = startPreviewMediaSync(sync, [video]);
  expect(sync).toHaveBeenCalledTimes(1);
  expect(frames.size).toBe(0);
  useEditorStore.setState({ currentTime: 25 });
  expect(sync).toHaveBeenCalledTimes(2);
  useEditorStore.setState({ isPlaying: true });
  expect(frames.size).toBe(1);
  for (let i = 0; i < 120; i++) {
    useEditorStore.setState({ currentTime: i * 1000 / 60 });
    nextFrame();
    expect(frames.size).toBe(1);
  }
  expect(sync).toHaveBeenCalledTimes(123);
  useEditorStore.setState({ isPlaying: false });
  expect(sync).toHaveBeenCalledTimes(124);
  expect(frames.size).toBe(0);
  stop();
  video.dispatchEvent(new Event('canplay'));
  useEditorStore.setState({ isPlaying: true });
  expect(sync).toHaveBeenCalledTimes(124);
  expect(frames.size).toBe(0);
});

it('finishes a paused seek after metadata and a superseded seek complete without polling', () => {
  const video = Object.assign(new EventTarget(), {
    currentTime: 0, playbackRate: 1, preservesPitch: true, paused: true,
    seeking: false, readyState: 0, muted: true, play: vi.fn(async () => {}), pause: vi.fn(),
  });
  const sync = createMediaSynchronizer(video, vi.fn());
  const stop = startPreviewMediaSync(() => {
    const state = useEditorStore.getState();
    sync({ sourceTime: state.currentTime, rate: 1, clipId: 'take', muted: false }, state.isPlaying, false, state.playbackRevision);
  }, [video]);
  useEditorStore.setState({ currentTime: 1000, playbackRevision: 1 });
  expect(video.currentTime).toBe(0);
  video.readyState = 4;
  video.dispatchEvent(new Event('loadedmetadata'));
  expect(video.currentTime).toBe(1);
  video.seeking = true;
  useEditorStore.setState({ currentTime: 1020, playbackRevision: 2 });
  useEditorStore.setState({ currentTime: 1040, playbackRevision: 3 });
  expect(video.currentTime).toBe(1);
  video.seeking = false;
  video.dispatchEvent(new Event('seeked'));
  expect(video.currentTime).toBe(1.04);
  expect(frames.size).toBe(0);
  stop();
});

it('cancels a playing loop and removes listeners on replacement', () => {
  useEditorStore.setState({ isPlaying: true });
  const video = new EventTarget();
  const sync = vi.fn();
  const stop = startPreviewMediaSync(sync, [video]);
  expect(frames.size).toBe(1);
  stop();
  nextFrame();
  for (const event of ['loadedmetadata', 'canplay', 'seeked']) video.dispatchEvent(new Event(event));
  expect(sync).toHaveBeenCalledTimes(1);
  expect(frames.size).toBe(0);
});
