import { Matrix3 } from 'three';
import type { SourceCrop } from '../sourceCrop';

/** Remove the displayed camera crop while keeping the recording's source crop. */
export function sourceTextureTransform(matrix: Matrix3, view: SourceCrop): Matrix3 {
  return matrix.clone().multiply(new Matrix3().set(
    1/view.width,0,-view.x/view.width,
    0,1/view.height,-(1-view.y-view.height)/view.height,
    0,0,1,
  ));
}
