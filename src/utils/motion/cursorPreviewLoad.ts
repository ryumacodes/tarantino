export async function loadCursorPreview(
  duration: number,
  currentTime: () => number,
  compute: (until?: number) => Promise<string>,
  publish: (result: string, complete: boolean) => void,
  cancelled: () => boolean,
): Promise<void> {
  let previewEnd = Math.ceil(Math.max(0, currentTime()) + 5000);
  while (previewEnd < duration) {
    const opening = await compute(previewEnd);
    if (cancelled()) return;
    publish(opening, false);
    const playhead = currentTime();
    if (playhead < previewEnd) break;
    previewEnd = Math.ceil(playhead + 5000);
  }
  const full = await compute();
  if (!cancelled()) publish(full, true);
}
