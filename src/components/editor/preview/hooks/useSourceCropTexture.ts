import { useMemo, type MutableRefObject } from 'react';
import { useFrame } from '@react-three/fiber';
import type { SourceCrop } from '../../../../utils/sourceCrop';
import type { Texture } from 'three';
import { useEditorStore } from '../../../../stores/editor';
import { normalizedCrop, resolveSourceCrop } from '../../../../utils/sourceCrop';

export function useSourceCropTexture(texture: Texture | null, viewRef?: MutableRefObject<SourceCrop>) {
  const sourceCrop = useEditorStore(state => state.visualSettings.sourceCrop);
  const width = useEditorStore(state => state.videoWidth);
  const height = useEditorStore(state => state.videoHeight);
  const rect = useMemo(() => sourceCrop && width && height
    ? normalizedCrop(resolveSourceCrop(sourceCrop,width,height)) : undefined,
  [sourceCrop,width,height]);
  useFrame(() => {
    if (!texture) return;
    const crop=rect ?? {x:0,y:0,width:1,height:1};
    const view=viewRef?.current ?? {x:0,y:0,width:1,height:1};
    const width=crop.width*view.width, height=crop.height*view.height;
    texture.repeat.set(width,height);
    texture.offset.set(crop.x+view.x*crop.width,1-crop.y-view.y*crop.height-height);
    texture.updateMatrix();
  },-1);
  return rect;
}
