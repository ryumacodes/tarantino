import { LatestTaskQueue } from '../../../utils/latestTaskQueue';

type Video = EventTarget & Pick<HTMLVideoElement, 'paused' | 'seeking' | 'requestVideoFrameCallback' | 'cancelVideoFrameCallback'>;

export function observePausedFrameRecovery<T>(video: Video, options: {
  request: () => Promise<T>;
  publish: (value: T) => void;
  dispose: (value: T) => void;
  clear: () => void;
  error: (error: unknown) => void;
}) {
  const queue = new LatestTaskQueue<T>();
  let generation = 0;
  let delivered = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const reset = () => {
    generation++;
    clearTimeout(timer);
    queue.clearPending();
    options.clear();
  };
  const seeking = () => { reset(); delivered = false; };
  const seeked = () => {
    clearTimeout(timer);
    if (!video.paused || delivered) return;
    const revision = generation;
    timer = setTimeout(() => {
      if (!video.paused || video.seeking || revision !== generation) return;
      void queue.run(options.request).then(value => {
        if (value === undefined) return;
        if (revision !== generation || !video.paused || video.seeking) options.dispose(value);
        else options.publish(value);
      }).catch(error => { if (revision === generation) options.error(error); });
    }, 250);
  };
  const decoded = () => {
    if (!video.seeking) { delivered = true; reset(); }
    frame = video.requestVideoFrameCallback(decoded);
  };
  let frame = video.requestVideoFrameCallback(decoded);
  video.addEventListener('seeking', seeking);
  video.addEventListener('seeked', seeked);
  video.addEventListener('play', reset);
  const stop = () => {
    reset();
    video.cancelVideoFrameCallback(frame);
    video.removeEventListener('seeking', seeking);
    video.removeEventListener('seeked', seeked);
    video.removeEventListener('play', reset);
  };
  return { cancel: reset, stop };
}
