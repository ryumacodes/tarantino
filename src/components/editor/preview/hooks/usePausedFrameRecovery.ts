import { useEffect, useState } from 'react';
import { useThree } from '@react-three/fiber';
import { invoke } from '@tauri-apps/api/core';
import { LinearFilter, SRGBColorSpace, Texture, TextureLoader } from 'three';
import { isMacRuntime } from '../../../../utils/platform';
import { useEditorStore } from '../../../../stores/editor';
import { previewSample } from '../../../../utils/timeline/previewTimeline';
import { observePausedFrameRecovery } from '../pausedFrameRecovery';

export function usePausedFrameRecovery(video: HTMLVideoElement) {
  const [recovered, setRecovered] = useState<Texture | null>(null);
  const invalidate = useThree(state => state.invalidate);
  useEffect(() => {
    if (!isMacRuntime || !video.requestVideoFrameCallback) return;
    let current: Texture | null = null;
    const clear = () => {
      if (!current) return;
      current.dispose(); current = null;
      setRecovered(null); invalidate(2);
    };
    const recovery = observePausedFrameRecovery(video, {
      async request() {
        const state = useEditorStore.getState();
        const sample = previewSample(state, state.currentTime);
        const path = sample.sourceFilePath ?? state.videoFilePath;
        if (!path || sample.sourceTime === null) throw new Error('No source frame at playhead');
        const image = await invoke<ArrayBuffer>('extract_paused_video_frame', {
          videoPath: path, sourceTimeMs: sample.sourceTime, width: Math.min(video.videoWidth, 8192),
        });
        const url = URL.createObjectURL(new Blob([image], { type: 'image/png' }));
        let texture: Texture;
        try { texture = await new TextureLoader().loadAsync(url); }
        finally { URL.revokeObjectURL(url); }
        texture.colorSpace = SRGBColorSpace;
        texture.minFilter = LinearFilter;
        texture.magFilter = LinearFilter;
        texture.generateMipmaps = false;
        return texture;
      },
      publish(texture) { clear(); current = texture; setRecovered(texture); invalidate(2); },
      clear,
      dispose(texture) { texture.dispose(); },
      error(error) { console.warn('Paused frame recovery failed:', error); },
    });
    const unsubscribe = useEditorStore.subscribe((state, previous) => {
      if (state.playbackRevision !== previous.playbackRevision) recovery.cancel();
    });
    return () => { unsubscribe(); recovery.stop(); };
  }, [video, invalidate]);
  return recovered;
}
