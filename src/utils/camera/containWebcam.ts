/** Overlay size is a fraction of canvas width; keep its square inside both axes. */
export function containWebcam(x: number, y: number, size: number, width: number, height: number) {
  const aspect = width > 0 && height > 0 ? width / height : 1;
  const fitted = Math.max(0, Math.min(size, 1, 1 / aspect));
  const halfX = fitted / 2;
  const halfY = fitted * aspect / 2;
  return {
    x: Math.max(halfX, Math.min(1 - halfX, x)),
    y: Math.max(halfY, Math.min(1 - halfY, y)),
    size: fitted,
  };
}
