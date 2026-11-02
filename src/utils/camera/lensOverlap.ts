export interface CameraRect { x: number; y: number; width: number; height: number }

/** Fraction of an axis-aligned camera rectangle inside a circular lens, in shared stage units. */
export function lensCameraCoverage(cx: number, cy: number, radius: number, rect: CameraRect): number {
  if (![cx, cy, radius, rect.x, rect.y, rect.width, rect.height].every(Number.isFinite)
    || radius <= 0 || rect.width <= 0 || rect.height <= 0) return 0;
  // Integrate in unit-circle coordinates so large stage resolutions do not square overflow.
  const x0=(rect.x-cx)/radius, x1=(rect.x+rect.width-cx)/radius;
  const y0=(rect.y-cy)/radius, y1=(rect.y+rect.height-cy)/radius;
  if (x0 >= 1 || x1 <= -1 || y0 >= 1 || y1 <= -1) return 0;
  const primitive = (x: number) => (x*Math.sqrt(Math.max(0,1-x*x))+Math.asin(x))/2;
  const quadrant = (x: number,y: number) => {
    const sign=Math.sign(x)*Math.sign(y);
    x=Math.min(1,Math.abs(x));y=Math.min(1,Math.abs(y));
    const flatEnd=Math.min(x,Math.sqrt(Math.max(0,1-y*y)));
    return sign*(flatEnd*y+primitive(x)-primitive(flatEnd));
  };
  const area=quadrant(x1,y1)-quadrant(x0,y1)-quadrant(x1,y0)+quadrant(x0,y0);
  return Math.max(0,Math.min(1,area/((rect.width/radius)*(rect.height/radius))));
}
