import { useEffect, useRef, useState } from 'react';
import { useEditorStore } from '../../../../stores/editor';
import { useEditGesture } from '../../../../hooks/useEditGesture';

export function useClipTrim(pixelsPerMs: number, isExporting: boolean) {
  const gesture = useEditGesture();
  const suppressClickUntil = useRef(0);
  useEffect(() => {
    const click = (event: MouseEvent) => {
      if (performance.now() <= suppressClickUntil.current) {
        event.preventDefault(); event.stopPropagation();
      }
      suppressClickUntil.current = 0;
    };
    document.addEventListener('click', click, true);
    return () => document.removeEventListener('click', click, true);
  }, []);
  const [drag, setDrag] = useState<{ id: string; side: 'start'|'end'; x: number; source: number; rate: number; scale: number } | null>(null);
  const begin = (event: React.MouseEvent, id: string, side: 'start'|'end') => {
    event.preventDefault(); event.stopPropagation();
    const state = useEditorStore.getState();
    const clip = state.clips.find(clip => clip.id === id);
    if (isExporting || !clip || clip.locked || state.tracks.find(track => track.id === clip.trackId)?.locked || pixelsPerMs <= 0) return;
    state.setIsPlaying(false);
    state.selectClips([id]);
    gesture.begin();
    setDrag({id,side,x:event.clientX,source:side==='start'?clip.sourceIn:clip.sourceOut,rate:clip.playbackRate,scale:pixelsPerMs});
  };
  useEffect(() => {
    if (!drag) return;
    const move = (event: MouseEvent) => {
      if (isExporting) return;
      useEditorStore.getState().trimClip(drag.id,drag.side,drag.source+(event.clientX-drag.x)/drag.scale*drag.rate);
    };
    const stop = (event: Event) => {
      // The edge moves under the pointer; its release click must not seek the timeline.
      suppressClickUntil.current = event.type === 'mouseup' ? performance.now()+100 : 0;
      gesture.end(); setDrag(null);
    };
    document.addEventListener('mousemove',move);
    document.addEventListener('mouseup',stop);
    window.addEventListener('blur',stop);
    return () => {
      document.removeEventListener('mousemove',move);
      document.removeEventListener('mouseup',stop);
      window.removeEventListener('blur',stop);
    };
  },[drag,isExporting,gesture.end]);
  return begin;
}
