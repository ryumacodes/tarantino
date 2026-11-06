import { beforeEach, expect, it } from 'vitest';
import { shallow } from 'zustand/shallow';
import { useEditorStore } from '../../../../stores/editor/store';
import { selectVideoInputs, selectVideoMaterialInputs } from './videoInputs';

const initial = useEditorStore.getState();
beforeEach(() => useEditorStore.setState(initial, true));
it('does not invalidate the video scene for playhead, selection, or mouse-data updates', () => {
  const selected = selectVideoInputs(useEditorStore.getState());
  for (let frame = 0; frame < 120; frame++) {
    useEditorStore.setState({ currentTime: frame * 1000 / 60, selectedBlockId: `block-${frame}`, mouseEvents: [] });
    expect(shallow(selected, selectVideoInputs(useEditorStore.getState()))).toBe(true);
  }
});
it('refreshes geometry when source dimensions or output dimensions change', () => {
  const before = selectVideoInputs(useEditorStore.getState());
  useEditorStore.setState({ videoWidth: 1024, videoHeight: 768 });
  const source = selectVideoInputs(useEditorStore.getState());
  expect(shallow(before, source)).toBe(false);
  useEditorStore.setState({ exportSettings: { ...initial.exportSettings, resolution: 'custom', customWidth: 900, customHeight: 1600 } });
  const output = selectVideoInputs(useEditorStore.getState());
  expect([output.outputWidth, output.outputHeight]).toEqual([900, 1600]);
  expect(shallow(source, output)).toBe(false);
});

it('isolates the canvas from playback ticks and updates its clock only on displayed seconds', async () => {
  const { selectPreviewInputs, selectPlaybackSecond } = await import('./videoInputs');
  const preview = selectPreviewInputs(useEditorStore.getState());
  let last = selectPlaybackSecond(useEditorStore.getState());
  let changes = 0;
  for (let frame = 1; frame <= 120; frame++) {
    useEditorStore.setState({ currentTime: frame * 1000 / 60 });
    const state = useEditorStore.getState();
    expect(shallow(preview, selectPreviewInputs(state))).toBe(true);
    const next = selectPlaybackSecond(state);
    if (last !== next) changes++;
    last = next;
  }
  expect(changes).toBe(2);
  useEditorStore.setState({ currentTime: 0 });
  expect(selectPlaybackSecond(useEditorStore.getState())).toBe(0);
});

it('keeps video material inputs stable during playback but detects gaps and audio edits', () => {
  const clip = { id: 'take', name: 'Take', type: 'video' as const, trackId: 'v',
    startTime: 0, duration: 1000, sourceIn: 0, sourceOut: 1000, playbackRate: 1, enabled: true, locked: false };
  const track = { id: 'v', name: 'Video', type: 'video' as const, clips: [clip], height: 60,
    visible: true, muted: false, solo: false, locked: false, order: 0 };
  useEditorStore.setState({ duration: 2000, clips: [clip], tracks: [track], currentTime: 0 });
  const selected = selectVideoMaterialInputs(useEditorStore.getState());
  for (let frame = 1; frame < 60; frame++) {
    useEditorStore.getState().setCurrentTime(frame * 1000 / 60, 'playback');
    expect(shallow(selected, selectVideoMaterialInputs(useEditorStore.getState()))).toBe(true);
  }
  useEditorStore.getState().setCurrentTime(1500);
  expect(selectVideoMaterialInputs(useEditorStore.getState()).gap).toBe(true);
  useEditorStore.getState().setCurrentTime(500);
  expect(shallow(selected, selectVideoMaterialInputs(useEditorStore.getState()))).toBe(true);
  useEditorStore.setState({ audioSettings: { ...initial.audioSettings, systemGain: 0.5 } });
  expect(shallow(selected, selectVideoMaterialInputs(useEditorStore.getState()))).toBe(false);
});
