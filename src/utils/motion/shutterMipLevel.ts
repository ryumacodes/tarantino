type Rect = readonly [number, number, number, number];

export function shutterMipLevel(source: readonly [number, number], before: Rect, now: Rect,
  output: readonly [number, number], budget: number, span: number): number {
  let footprint = Math.max(source[0] / Math.min(before[2], now[2]),
    source[1] / Math.min(before[3], now[3]));
  if (before.some((value, index) => value !== now[index])) {
    let travel = 0;
    for (const x of [0, output[0]]) for (const y of [0, output[1]]) {
      const dx = ((x - before[0]) / before[2] - (x - now[0]) / now[2]) * source[0];
      const dy = ((y - before[1]) / before[3] - (y - now[1]) / now[3]) * source[1];
      travel = Math.max(travel, Math.hypot(dx, dy));
    }
    // Uncapped tap spacing is at most span pixels; capped spacing is travel/budget.
    footprint = Math.max(footprint, span * Math.max(source[0] / before[2], source[1] / before[3]), travel / budget);
  }
  return Math.floor(Math.max(0, Math.log2(footprint))) + 1;
}
