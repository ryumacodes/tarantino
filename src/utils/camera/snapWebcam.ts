interface Point { x: number; y: number }
/** Snap only within the attraction radius; otherwise keep the contained point. */
export function snapWebcam(point: Point, corners: readonly Point[], threshold: number): Point {
  let closest: Point | undefined;
  let distance = threshold;
  for (const corner of corners) {
    const candidate = Math.hypot(corner.x - point.x, corner.y - point.y);
    if (candidate < distance) { closest = corner; distance = candidate; }
  }
  return closest ?? point;
}
