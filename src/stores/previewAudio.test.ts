import { expect, it, vi } from 'vitest';
import { subscribePreviewMute, usePreviewAudioStore } from './previewAudio';
import { useEditorStore } from './editor/store';

it('applies mute on attachment and toggles all listeners without modifying project audio', () => {
  usePreviewAudioStore.setState({ muted: true });
  const settings = useEditorStore.getState().audioSettings;
  const tracks = vi.fn(), music = vi.fn();
  const stopTracks = subscribePreviewMute(tracks);
  const stopMusic = subscribePreviewMute(music);
  expect(tracks).toHaveBeenLastCalledWith(true);
  expect(music).toHaveBeenLastCalledWith(true);
  usePreviewAudioStore.getState().toggle();
  expect(tracks).toHaveBeenLastCalledWith(false);
  expect(music).toHaveBeenLastCalledWith(false);
  expect(useEditorStore.getState().audioSettings).toBe(settings);
  stopTracks(); stopMusic();
  usePreviewAudioStore.getState().toggle();
  expect(tracks).toHaveBeenCalledTimes(2);
  expect(music).toHaveBeenCalledTimes(2);
});
