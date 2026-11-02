import { expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../store';
import { createZoomActions } from './zoomActions';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

it('preserves saved canvas and window layout when pointer metadata loads', async () => {
  vi.mocked(invoke).mockResolvedValue(JSON.stringify({
    capture_mode: 'window', display_width: 800, display_height: 600,
    screen_width: 1470, screen_height: 956,
    recording_area: { x: 20, y: 33, width: 800, height: 600 },
    mouse_events: [{ base: { timestamp: 500, x: 100, y: 200, event_type: 'Move' } }],
  }));
  const state = { ...useEditorStore.getInitialState(), videoWidth: 1600, videoHeight: 1200,
    visualSettings: { ...useEditorStore.getInitialState().visualSettings, windowLayoutMode: 'desktop' as const, aspectRatio: '9:16' as const } };
  const actions = createZoomActions(recipe => recipe(state), () => state);
  await actions.loadMouseEvents('recording.mouse.json');
  expect(state.visualSettings).toEqual({ ...useEditorStore.getInitialState().visualSettings, windowLayoutMode: 'desktop', aspectRatio: '9:16' as const });
  expect(state.screenResolution).toEqual({ width: 1470, height: 956 });
  expect(state.mouseEvents).toHaveLength(1);
  expect(state.mouseEventsLoading).toBe(false);
});
