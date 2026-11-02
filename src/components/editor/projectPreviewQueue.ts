import { invoke } from '@tauri-apps/api/core';

type Request = { path: string; listeners: Set<(frame: string) => void>; running: boolean };
/** Only queued work can be cancelled; active native decoders finish into the cache. */
export function createPreviewQueue(decode: (path: string) => Promise<string>, concurrency = 2, capacity = 80) {
  const cache = new Map<string, string>();
  const requests = new Map<string, Request>();
  const queue: Request[] = [];
  let running = 0;
  const pump = () => {
    while (running < concurrency && queue.length) {
      const request = queue.shift()!;
      request.running = true;
      running++;
      void Promise.resolve().then(() => decode(request.path)).then(frame => {
        if (frame) {
          cache.set(request.path, frame);
          if (cache.size > capacity) cache.delete(cache.keys().next().value!);
        }
        return frame;
      }, () => '').then(frame => {
        requests.delete(request.path);
        for (const listener of request.listeners) listener(frame);
      }).finally(() => { running--; pump(); });
    }
  };
  return (path: string, listener: (frame: string) => void): (() => void) => {
    const cached = cache.get(path);
    if (cached) {
      cache.delete(path); cache.set(path, cached);
      listener(cached);
      return () => {};
    }
    let request = requests.get(path);
    if (!request) {
      request = { path, listeners: new Set(), running: false };
      requests.set(path, request);
      queue.push(request);
    }
    // Separate subscriptions remain independent even if callers reuse a callback.
    const notify = (frame: string) => listener(frame);
    request.listeners.add(notify);
    pump();
    return () => {
      if (!request.listeners.delete(notify)) return;
      if (!request.running && request.listeners.size === 0) {
        queue.splice(queue.indexOf(request), 1);
        requests.delete(path);
      }
    };
  };
}

export const requestProjectPreview = createPreviewQueue(async path => {
  const frames = await invoke<string[]>('extract_video_thumbnails', {
    videoPath: path, thumbnailCount: 1, thumbnailWidth: 480,
  });
  return frames[0] ?? '';
});
