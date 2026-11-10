import { useEditorStore } from '../../../stores/editor';

export function startPreviewMediaSync(sync: () => void, media: readonly EventTarget[]) {
  let frame: number | undefined;
  const refresh = () => {
    sync();
    if (useEditorStore.getState().isPlaying) {
      if (frame === undefined) frame = requestAnimationFrame(tick);
    } else if (frame !== undefined) {
      cancelAnimationFrame(frame);
      frame = undefined;
    }
  };
  const tick = () => { frame = undefined; refresh(); };
  const unsubscribe = useEditorStore.subscribe((state, previous) => {
    if (!state.isPlaying || state.isPlaying !== previous.isPlaying) refresh();
  });
  const events = ['loadedmetadata', 'canplay', 'seeked'];
  for (const element of media) for (const event of events) element.addEventListener(event, refresh);
  refresh();
  return () => {
    unsubscribe();
    if (frame !== undefined) cancelAnimationFrame(frame);
    for (const element of media) for (const event of events) element.removeEventListener(event, refresh);
  };
}
