interface SeekUpdates {
  subscribe: (listener: () => void) => () => void;
  restart: () => Promise<void>;
}

export async function loadCursorPreview(
  duration: number,
  currentTime: () => number,
  compute: (until?: number) => Promise<string>,
  publish: (result: string, complete: boolean) => void,
  cancelled: () => boolean,
  seeks?: SeekUpdates,
): Promise<void> {
  let coveredEnd = 0;
  while (!cancelled()) {
    let previewEnd = Math.ceil(Math.max(0, currentTime()) + 5000);
    while (previewEnd < duration) {
      const opening = await compute(previewEnd);
      if (cancelled()) return;
      publish(opening, false);
      coveredEnd = previewEnd;
      const playhead = currentTime();
      if (playhead < previewEnd) break;
      previewEnd = Math.ceil(playhead + 5000);
    }
    let restart: Promise<{error: unknown} | null> | undefined;
    const unsubscribe = seeks?.subscribe(() => {
      const time = currentTime();
      if (!restart && !cancelled() && time >= coveredEnd && time + 5000 < duration) {
        restart = seeks.restart().then(() => null, error => ({error}));
      }
    });
    try {
      const full = await compute();
      if (!restart && !cancelled()) publish(full, true);
    } catch (error) {
      if (!restart) throw error;
    } finally {
      unsubscribe?.();
    }
    if (!restart) return;
    const failure = await restart;
    if (failure) throw failure.error;
  }
}
