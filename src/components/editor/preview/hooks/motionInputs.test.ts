import { beforeEach, expect, it } from 'vitest';
import { shallow } from 'zustand/shallow';
import { useEditorStore } from '../../../../stores/editor/store';
import { selectMotionInputs } from './motionInputs';

const initial = useEditorStore.getState();
beforeEach(() => useEditorStore.setState(initial, true));
it('keeps motion subscriptions stable through playback and selection changes', () => {
  const before = selectMotionInputs(useEditorStore.getState());
  for (let tick = 0; tick < 120; tick++) {
    useEditorStore.setState({ currentTime: tick * 1000 / 60, selectedBlockId: `block-${tick}` });
    expect(shallow(before, selectMotionInputs(useEditorStore.getState()))).toBe(true);
  }
});
it('includes output dimensions and motion edits in the selected inputs', () => {
  const before = selectMotionInputs(useEditorStore.getState());
  useEditorStore.setState({ exportSettings: { ...initial.exportSettings, resolution: 'custom', customWidth: 800, customHeight: 600 } });
  const resized = selectMotionInputs(useEditorStore.getState());
  expect([resized.outputWidth, resized.outputHeight]).toEqual([800, 600]);
  expect(shallow(before, resized)).toBe(false);
  useEditorStore.setState({ visualSettings: { ...initial.visualSettings, screenMotionStyle: 'smooth' } });
  expect(shallow(resized, selectMotionInputs(useEditorStore.getState()))).toBe(false);
});

it('invalidates camera preparation when playback bounds or clips change', () => {
  const before = selectMotionInputs(useEditorStore.getState());
  useEditorStore.setState({ trimStart: 200, trimEnd: 700 });
  const trimmed = selectMotionInputs(useEditorStore.getState());
  expect(shallow(before, trimmed)).toBe(false);
  expect([trimmed.trimStart, trimmed.trimEnd]).toEqual([200, 700]);
  useEditorStore.setState({ clips: [...initial.clips] });
  expect(shallow(trimmed, selectMotionInputs(useEditorStore.getState()))).toBe(false);
});
