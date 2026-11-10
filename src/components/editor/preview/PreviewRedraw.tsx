import { useEffect } from 'react';
import { useThree } from '@react-three/fiber';
import { subscribePreviewRedraw } from './previewRedrawEvents';

export function PreviewRedraw() {
  const invalidate = useThree(state => state.invalidate);
  useEffect(() => subscribePreviewRedraw(invalidate), [invalidate]);
  useEffect(() => { invalidate(2); });
  return null;
}
