import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_EXPORT_SETTINGS, DEFAULT_VISUAL_SETTINGS } from '../constants';
import { createSettingsActions } from './settingsActions';

const createHarness = (overrides: Record<string, unknown> = {}) => {
  const state: any = {
    videoWidth: 1920,
    videoHeight: 1080,
    visualSettings: { ...DEFAULT_VISUAL_SETTINGS },
    exportSettings: { ...DEFAULT_EXPORT_SETTINGS },
    overlays: [],
    history: [],
    historyIndex: -1,
    ...overrides,
  };
  const set = (recipe: (draft: any) => void) => recipe(state);
  const actions = createSettingsActions(set, () => state as any);
  return { state, actions };
};

describe('getExportDimensions', () => {
  it('uses the source aspect for automatic output', () => {
    const { actions } = createHarness({ videoWidth: 1440, videoHeight: 900 });

    expect(actions.getExportDimensions()).toEqual({ width: 1728, height: 1080 });
  });

  it('uses an explicitly selected aspect ratio', () => {
    const { state, actions } = createHarness();
    state.visualSettings.aspectRatio = '9:16';

    expect(actions.getExportDimensions()).toEqual({ width: 608, height: 1080 });
  });

  it('honors explicit custom dimensions', () => {
    const { state, actions } = createHarness();
    Object.assign(state.exportSettings, {
      resolution: 'custom',
      customWidth: 1234,
      customHeight: 777,
    });

    expect(actions.getExportDimensions()).toEqual({ width: 1234, height: 777 });
  });

  it('rounds source-aspect output width to an even number', () => {
    const { actions } = createHarness({ videoWidth: 853, videoHeight: 480 });

    expect(actions.getExportDimensions().width % 2).toBe(0);
  });
});

describe('visual setting actions', () => {
  it('applies gradient wallpapers as normalized stops', () => {
    const { state, actions } = createHarness();
    actions.applyWallpaper('gradient-sunset');

    expect(state.visualSettings.backgroundType).toBe('wallpaper');
    expect(state.visualSettings.gradientStops).toEqual([
      { color: '#ff6b6b', position: 0 },
      { color: '#feca57', position: 50 },
      { color: '#ff9ff3', position: 100 },
    ]);
  });

  it('does not mutate settings for an unknown wallpaper', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const { state, actions } = createHarness();
    const before = structuredClone(state.visualSettings);
    actions.applyWallpaper('missing-wallpaper');

    expect(state.visualSettings).toEqual(before);
    warn.mockRestore();
  });

  it('resets visual settings without sharing the top-level defaults object', () => {
    const { state, actions } = createHarness();
    state.visualSettings.padding = 35;
    actions.resetVisualSettings();

    expect(state.visualSettings).toEqual(DEFAULT_VISUAL_SETTINGS);
    expect(state.visualSettings).not.toBe(DEFAULT_VISUAL_SETTINGS);
  });
});
