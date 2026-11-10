import { beforeEach, expect, it } from 'vitest';
import { shallow } from 'zustand/shallow';
import { useEditorStore } from '../../../../stores/editor/store';
import { selectWebcamInputs } from './webcamInputs';

const initial = useEditorStore.getState();
beforeEach(() => useEditorStore.setState(initial, true));

it('keeps a static webcam stable through 1200 playback ticks and unrelated edits', () => {
  useEditorStore.setState({ duration: 60000, webcamKeyframes: [], clips: [], tracks: [] });
  let previous = selectWebcamInputs(useEditorStore.getState(), []);
  let updates = 0;
  const unsubscribe = useEditorStore.subscribe(state => {
    const next = selectWebcamInputs(state, []);
    if (!shallow(previous, next)) updates++;
    previous = next;
  });
  try {
    for (let frame = 1; frame <= 1200; frame++) {
      useEditorStore.getState().setCurrentTime(frame * 1000 / 60, 'playback');
    }
    useEditorStore.setState({ selectedBlockId: 'zoom', mouseEvents: [] });
    expect(updates).toBe(0);
    useEditorStore.getState().updateVisualSettings({ webcamMirrored: !initial.visualSettings.webcamMirrored });
    expect(updates).toBe(1);
  } finally { unsubscribe(); }
});

it('updates camera state at edited source-time keyframes, silence boundaries and gaps', () => {
  const clip = { id: 'take', name: 'Take', type: 'video' as const, trackId: 'v',
    startTime: 0, duration: 1000, sourceIn: 2000, sourceOut: 4000, playbackRate: 2, enabled: true, locked: false };
  const track = { id: 'v', name: 'Video', type: 'video' as const, clips: [clip], height: 60,
    visible: true, muted: false, solo: false, locked: false, order: 0 };
  useEditorStore.setState({ duration: 5000, clips: [clip], tracks: [track], currentTime: 0,
    visualSettings: { ...initial.visualSettings, webcamEnabled: true },
    webcamKeyframes: [
      { id: 'a', time: 2000, x: .2, y: .3, size: .2, shape: 'circle', visible: true },
      { id: 'b', time: 3000, x: .7, y: .8, size: .3, shape: 'roundrect', visible: true },
    ] });
  const silence = [{ start_ms: 3200, end_ms: 3400 }];
  const sample = (time: number) => {
    useEditorStore.getState().setCurrentTime(time);
    return selectWebcamInputs(useEditorStore.getState(), silence);
  };
  expect(sample(499)).toMatchObject({ keyframeId: 'a', x: .2, shape: 'circle', gap: false });
  expect(sample(500)).toMatchObject({ keyframeId: 'b', x: .7, shape: 'roundrect', visible: true });
  expect(sample(600).visible).toBe(false);
  expect(sample(700).visible).toBe(true);
  expect(sample(1001).gap).toBe(true);
  expect(sample(200).keyframeId).toBe('a');
  useEditorStore.getState().updateWebcamKeyframe('a', { x: .4 });
  expect(sample(200).x).toBe(.4);
});
