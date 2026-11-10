import { useEditorStore } from '../../../stores/editor';

export function subscribePreviewRedraw(invalidate: (frames?: number) => void) {
  return useEditorStore.subscribe(state => {
    if (!state.isPlaying) invalidate(2);
  });
}

type FrameSource = EventTarget & Partial<Pick<HTMLVideoElement,
  'paused' | 'requestVideoFrameCallback' | 'cancelVideoFrameCallback'>>;

export function observeDecodedFrame(video: FrameSource, refresh: () => void) {
  let frame: number | undefined;
  const decoded = () => {
    // A seek event can precede the compositor receiving the new frame.
    if (video.paused) refresh();
    frame = video.requestVideoFrameCallback!(decoded);
  };
  if (video.requestVideoFrameCallback) frame = video.requestVideoFrameCallback(decoded);
  video.addEventListener('loadeddata', refresh);
  video.addEventListener('seeked', refresh);
  return () => {
    if (frame !== undefined) video.cancelVideoFrameCallback?.(frame);
    video.removeEventListener('loadeddata', refresh);
    video.removeEventListener('seeked', refresh);
  };
}
