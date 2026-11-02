import type { ZoomAnalysis, ZoomBlock } from './types';

/** Preserve manual ranges, splitting regenerated automatic coverage around them. */
export function mergeRebuiltZooms(analysis: ZoomAnalysis, previous: ZoomAnalysis | null): ZoomAnalysis {
  const manual = previous?.zoom_blocks.filter(block => block.is_manual) ?? [];
  const automatic: ZoomBlock[] = [];
  for (const block of analysis.zoom_blocks) {
    let ranges = [[block.start_time,block.end_time]];
    for (const saved of manual) ranges = ranges.flatMap(([start,end]) => {
      if (end <= saved.start_time || start >= saved.end_time) return [[start,end]];
      return [[start,Math.min(end,saved.start_time)],[Math.max(start,saved.end_time),end]]
        .filter(([a,b]) => b>a);
    });
    for (const [start,end] of ranges) if (end-start>500) automatic.push({
      ...block, id:`auto-${crypto.randomUUID()}`, start_time:start, end_time:end,
    });
  }
  return {...analysis,zoom_blocks:[...manual,...automatic].sort((a,b)=>a.start_time-b.start_time)};
}
