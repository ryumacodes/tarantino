import { beforeEach, expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { useEditorStore } from './store';

const initial = useEditorStore.getState();
beforeEach(() => useEditorStore.setState(initial));

it.each([8000, 12000])('keeps an untrimmed end aligned when metadata corrects duration to %i', duration => {
  useEditorStore.setState({ duration: 10000, trimStart: 0, trimEnd: 10000 });
  useEditorStore.getState().setDuration(duration);
  expect(useEditorStore.getState()).toMatchObject({ duration, trimStart: 0, trimEnd: duration });
});

it('preserves a deliberately trimmed range when metadata changes', () => {
  useEditorStore.setState({ duration: 10000, trimStart: 1000, trimEnd: 6000 });
  useEditorStore.getState().setDuration(12000);
  expect(useEditorStore.getState()).toMatchObject({ duration: 12000, trimStart: 1000, trimEnd: 6000 });
});

it('initializes the trim end when duration first becomes available', () => {
  useEditorStore.getState().setDuration(12000);
  expect(useEditorStore.getState()).toMatchObject({ duration: 12000, trimEnd: 12000 });
});
