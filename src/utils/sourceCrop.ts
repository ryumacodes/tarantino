/** Source-pixel crop geometry, separate from capture bounds and output padding. */
export interface SourceCrop { x: number; y: number; width: number; height: number }
export interface PixelCrop extends SourceCrop { sourceWidth: number; sourceHeight: number }

export function resolveSourceCrop(crop: SourceCrop, sourceWidth: number, sourceHeight: number): PixelCrop {
  if (!Number.isInteger(sourceWidth) || !Number.isInteger(sourceHeight) || sourceWidth <= 0 || sourceHeight <= 0
      || ![crop.x, crop.y, crop.width, crop.height].every(Number.isFinite)
      || crop.x < 0 || crop.y < 0 || crop.width <= 0 || crop.height <= 0
      || crop.x + crop.width > 1 + 1e-9 || crop.y + crop.height > 1 + 1e-9) {
    throw new Error('Crop must be a nonempty rectangle inside the source video');
  }
  if (crop.x >= 1 || crop.y >= 1) throw new Error('Crop must intersect the source video');
  return { x: crop.x * sourceWidth, y: crop.y * sourceHeight,
    width: crop.width * sourceWidth, height: crop.height * sourceHeight, sourceWidth, sourceHeight };
}

export function normalizedCrop(crop: PixelCrop): SourceCrop {
  return { x: crop.x / crop.sourceWidth, y: crop.y / crop.sourceHeight,
    width: crop.width / crop.sourceWidth, height: crop.height / crop.sourceHeight };
}

/** Outside positions remain outside; never pin an invisible pointer to the crop edge. */
export function cropSourcePoint(crop: PixelCrop, x: number, y: number) {
  return { x: (x * crop.sourceWidth - crop.x) / crop.width,
    y: (y * crop.sourceHeight - crop.y) / crop.height };
}
