// Settings Actions
import type {
  EditorState,
  EditorActions,
  Overlay,
  AudioSettings,
  VisualSettings,
  ExportSettings,
} from '../types';

import {
  DEFAULT_VISUAL_SETTINGS,
  DEFAULT_EXPORT_SETTINGS,
  RESOLUTION_DIMENSIONS,
  WALLPAPERS,
} from '../constants';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

export const createSettingsActions = (set: SetFn, get: GetFn) => ({
  // Overlay Actions
  addOverlay: (overlay: Overlay) => set((state) => {
    state.overlays.push(overlay);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  updateOverlay: (id: string, updates: Partial<Overlay>) => set((state) => {
    const index = state.overlays.findIndex(o => o.id === id);
    if (index !== -1) {
      Object.assign(state.overlays[index], updates);
      state.history.push({ ...state });
      state.historyIndex++;
    }
  }),

  deleteOverlay: (id: string) => set((state) => {
    state.overlays = state.overlays.filter(o => o.id !== id);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  // Audio Settings Actions
  updateAudioSettings: (settings: Partial<AudioSettings>) => set((state) => {
    Object.assign(state.audioSettings, settings);
  }),

  // Visual Settings Actions
  updateVisualSettings: (settings: Partial<VisualSettings>) => set((state) => {
    Object.assign(state.visualSettings, settings);
    console.log('Visual settings updated:', settings);
  }),

  resetVisualSettings: () => set((state) => {
    state.visualSettings = { ...DEFAULT_VISUAL_SETTINGS };
    console.log('Visual settings reset to defaults');
  }),

  applyWallpaper: (wallpaperId: string) => set((state) => {
    const wallpaper = WALLPAPERS[wallpaperId as keyof typeof WALLPAPERS];
    if (!wallpaper) {
      console.warn('Unknown wallpaper:', wallpaperId);
      return;
    }

    state.visualSettings.wallpaperId = wallpaperId;

    if (wallpaper.type === 'gradient') {
      state.visualSettings.backgroundType = 'gradient';
      const colors = [...(wallpaper as { type: 'gradient'; colors: readonly string[] }).colors];
      state.visualSettings.gradientStops = colors.map((color, i) => ({
        color,
        position: (i / (colors.length - 1)) * 100
      }));
    } else if (wallpaper.type === 'solid') {
      state.visualSettings.backgroundType = 'solid';
      state.visualSettings.backgroundColor = (wallpaper as { type: 'solid'; color: string }).color;
    }

    console.log('Applied wallpaper:', wallpaperId, wallpaper);
  }),

  // Export Settings Actions
  updateExportSettings: (settings: Partial<ExportSettings>) => set((state) => {
    Object.assign(state.exportSettings, settings);
    console.log('Export settings updated:', settings);
  }),

  resetExportSettings: () => set((state) => {
    state.exportSettings = { ...DEFAULT_EXPORT_SETTINGS };
    console.log('Export settings reset to defaults');
  }),

  getExportDimensions: () => {
    const state = get();
    const { resolution, customWidth, customHeight } = state.exportSettings;

    if (resolution === 'custom' && customWidth && customHeight) {
      return { width: customWidth, height: customHeight };
    }

    return RESOLUTION_DIMENSIONS[resolution as keyof typeof RESOLUTION_DIMENSIONS] || RESOLUTION_DIMENSIONS['1080p'];
  }
});
