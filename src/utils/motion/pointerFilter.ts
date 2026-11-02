export interface PointerSample {
  timestamp: number;
  x: number;
  y: number;
  event_type?: string | Record<string, unknown>;
}

/** Remove isolated jumps in logical source pixels before normalization. */
export function removePointerShakes<T extends PointerSample>(events: T[], threshold: number): T[] {
  if (!Number.isFinite(threshold) || threshold <= 0) return events;
  const ordered = [...events].sort((a, b) => a.timestamp - b.timestamp);
  let skipRelease = false;
  return ordered.filter((event, index) => {
    const hasType = (name: string) => event.event_type === name
      || (typeof event.event_type === 'object' && event.event_type !== null && name in event.event_type);
    if (skipRelease && hasType('ButtonRelease')) { skipRelease = false; return false; }
    const isolated = [ordered[index + 1], ordered[index + 2]].every(next => next
      && Math.abs(event.x - next.x) + Math.abs(event.y - next.y) > threshold);
    if (isolated && hasType('ButtonPress')) skipRelease = true;
    return !isolated;
  });
}
