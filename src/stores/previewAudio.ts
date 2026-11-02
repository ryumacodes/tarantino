import { create } from 'zustand';

/** Listening preference only: never saved into project or export settings. */
export const usePreviewAudioStore = create<{
  muted: boolean;
  toggle: () => void;
}>(set => ({ muted: true, toggle: () => set(state => ({ muted: !state.muted })) }));

export function subscribePreviewMute(apply: (muted: boolean) => void) {
  apply(usePreviewAudioStore.getState().muted);
  return usePreviewAudioStore.subscribe((state, previous) => {
    if (state.muted !== previous.muted) apply(state.muted);
  });
}
