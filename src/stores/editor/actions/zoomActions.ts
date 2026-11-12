import { rememberEditedZoomStyle } from '../../../utils/motion/zoomStylePreference';
import { MIN_ZOOM_DURATION_MS } from '../../../utils/timeline/zoomTiming';
import { shortcutsFromSidecar } from "../../../utils/shortcuts/sidecar";
import { typingRanges } from "../../../utils/timeline/typingRanges";
import { createRecordingLoadGuard } from '../recordingLoadGuard';
import { edit } from '../history';
import { mergeRebuiltZooms } from '../rebuildZooms';

import { invoke } from '@tauri-apps/api/core';
import type {
  EditorState,
  EditorActions,
  ZoomKeyframe,
  WebcamKeyframe,
  ZoomBlock,
  ZoomAnalysis,
  PreviewZoomAnalysis,
  MouseEventData,
} from '../types';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

export const createZoomActions = (set: SetFn, get: GetFn) => {
  const beginLoad = createRecordingLoadGuard(get);
  return ({

  addZoomKeyframe: (keyframe: ZoomKeyframe) => edit(set, (state) => {
    state.zoomKeyframes.push(keyframe);
    state.selectedKeyframe = keyframe;
  }),

  updateZoomKeyframe: (id: string, updates: Partial<ZoomKeyframe>) => edit(set, (state) => {
    const index = state.zoomKeyframes.findIndex(kf => kf.id === id);
    if (index !== -1) {
      Object.assign(state.zoomKeyframes[index], updates);
    }
  }),

  deleteZoomKeyframe: (id: string) => edit(set, (state) => {
    state.zoomKeyframes = state.zoomKeyframes.filter(kf => kf.id !== id);
    if (state.selectedKeyframe?.id === id) {
      state.selectedKeyframe = null;
    }
  }),

  addWebcamKeyframe: (keyframe: WebcamKeyframe) => edit(set, (state) => {
    state.webcamKeyframes.push(keyframe);
  }),

  updateWebcamKeyframe: (id: string, updates: Partial<WebcamKeyframe>) => edit(set, (state) => {
    const index = state.webcamKeyframes.findIndex(kf => kf.id === id);
    if (index !== -1) {
      Object.assign(state.webcamKeyframes[index], updates);
    }
  }),

  deleteWebcamKeyframe: (id: string) => edit(set, (state) => {
    state.webcamKeyframes = state.webcamKeyframes.filter(kf => kf.id !== id);
  }),

  regenerateZooms: async () => {
    const { videoFilePath, zoomAnalysis: before } = get();
    if (!videoFilePath) throw new Error('Open a recording first.');
    const current = beginLoad('loadZoomData');
    const analysis = await invoke<ZoomAnalysis>('regenerate_auto_zoom_data', { videoPath: videoFilePath });
    if (!current()) return false;
    if (get().zoomAnalysis !== before) throw new Error('Zooms changed during analysis. Try again to use your latest edits.');
    edit(set, state => {
      state.zoomAnalysis = mergeRebuiltZooms(analysis,before);
      state.selectedBlockId = null;
      state.zoomLoading = false;
    });
    return true;
  },
  loadZoomData: async (filePath: string) => {
    if (get().videoFilePath && get().videoFilePath !== filePath) return;
    const current = beginLoad('loadZoomData');
    const originalAnalysis = get().zoomAnalysis;
    set((state) => { state.zoomLoading = true; });

    try {
      const analysis = await invoke<ZoomAnalysis | null>('load_auto_zoom_data', {
        videoPath: filePath
      });

      if (!current()) return;
      if (get().zoomAnalysis !== originalAnalysis) {
        set(state => { state.zoomLoading = false; });
        return;
      }
      set((state) => {
        if (analysis && analysis.zoom_blocks) {
          const videoDuration = state.duration;
          analysis.zoom_blocks = analysis.zoom_blocks
            .map(block => ({
              ...block,
              end_time: Math.min(block.end_time, videoDuration),
            }))
            .filter(block => block.end_time > block.start_time);
          analysis.session_duration = videoDuration;
        }
        state.zoomAnalysis = analysis;
        state.zoomLoading = false;
      });
    } catch (error) {
      if (!current()) return;
      console.error('[ZOOM] Failed to load zoom data:', error);

      set((state) => {
        if (get().zoomAnalysis === originalAnalysis) state.zoomAnalysis = null;
        state.zoomLoading = false;
      });
    }
  },

  updateZoomBlock: (blockId: string, updates: Partial<ZoomBlock>) => {
    const original = get().zoomAnalysis?.zoom_blocks.find(block => block.id === blockId);
    if (!original) return;
    const before = { ...original, loupe: original.loupe ? { ...original.loupe } : null };
    edit(set, (state) => {
      const block = state.zoomAnalysis!.zoom_blocks.find(block => block.id === blockId)!;

      const clampedUpdates = {
        ...updates,
        ...(updates.end_time !== undefined && {
          end_time: Math.min(updates.end_time, state.duration),
        }),
      };
      Object.assign(block, clampedUpdates);

      if (updates.start_time !== undefined || updates.end_time !== undefined) {
        block.timing_adjusted = true;
      }

      if (updates.is_manual === undefined && (updates.center_x !== undefined || updates.center_y !== undefined)) {
        block.is_manual = true;
      }
    });
    const after = get().zoomAnalysis!.zoom_blocks.find(block => block.id === blockId)!;
    rememberEditedZoomStyle(before, after);
  },

  deleteZoomBlock: (blockId: string) => edit(set, (state) => {
    if (!state.zoomAnalysis) return;

    state.zoomAnalysis.zoom_blocks = state.zoomAnalysis.zoom_blocks.filter(
      block => block.id !== blockId
    );

    if (state.selectedBlockId === blockId) {
      state.selectedBlockId = null;
    }
  }),

  addZoomBlock: (block: ZoomBlock) => edit(set, (state) => {
    if (!state.zoomAnalysis) {
      state.zoomAnalysis = {
        zoom_blocks: [],
        total_clicks: 0,
        session_duration: state.duration,
        config: {
          enabled: true,
          zoom_factor: 2.0,
          zoom_duration: 1000,
          min_click_spacing: 1000
        }
      };
    }

    const clampedBlock = {
      ...block,
      end_time: Math.min(block.end_time, state.duration),
    };

    state.zoomAnalysis.zoom_blocks.push(clampedBlock);
  }),

  splitZoomBlockAtTime: (blockId: string, time: number) => edit(set, (state) => {
    if (!state.zoomAnalysis) return;

    const block = state.zoomAnalysis.zoom_blocks.find(block => block.id === blockId);
    if (!block || time - block.start_time < MIN_ZOOM_DURATION_MS
      || block.end_time - time < MIN_ZOOM_DURATION_MS) return;

    const originalCenters = block.centers ?? [];
    let firstCenters = originalCenters.filter(center => center.time < time);
    let secondCenters = originalCenters.filter(center => center.time >= time);
    if (block.kind === 'typing' && !block.is_manual) {
      const sorted = [...originalCenters].sort((a, b) => a.time - b.time);
      const nextIndex = sorted.findIndex(center => center.time >= time);
      const boundaryIndex = nextIndex < 0 ? sorted.length : nextIndex;
      const next = sorted[boundaryIndex];
      const previous = sorted[boundaryIndex - 1];
      const anchor = next?.time === time ? next : previous;
      if (anchor) {
        const progress = next && previous && next.time > time
          ? (time - previous.time) / (next.time - previous.time) : 0;
        const boundary = { time, x: anchor.x + ((next?.x ?? anchor.x) - anchor.x) * progress,
          y: anchor.y + ((next?.y ?? anchor.y) - anchor.y) * progress };
        firstCenters = [...firstCenters, boundary];
        if (next?.time !== time) secondCenters = [boundary, ...secondCenters];
      }
    }
    const secondBlock = {
      ...block,
      id: crypto.randomUUID(),
      start_time: time,
      centers: secondCenters,
      timing_adjusted: true,
    };
    block.end_time = time;
    block.centers = firstCenters;
    block.timing_adjusted = true;
    state.zoomAnalysis.zoom_blocks.push(secondBlock);

    state.zoomAnalysis.zoom_blocks.sort((a, b) => a.start_time - b.start_time);
  }),

  saveZoomData: async () => {
    const state = get();
    if (!state.videoFilePath || !state.zoomAnalysis) {
      console.warn('Cannot save zoom data: missing video path or analysis data');
      return;
    }

    try {
      await invoke('save_auto_zoom_data', {
        videoPath: state.videoFilePath,
        analysis: state.zoomAnalysis
      });
    } catch (error) {
      console.error('Failed to save zoom data:', error);
      throw error;
    }
  },

  setSelectedBlockId: (id: string | null) => set((state) => {
    state.selectedBlockId = id;
  }),

  loadPreviewZoomData: async (filePath: string) => {
    if (get().videoFilePath && get().videoFilePath !== filePath) return;
    const current = beginLoad('loadPreviewZoomData');
    set((state) => { state.previewZoomLoading = true; });

    try {
      const analysis = await invoke<PreviewZoomAnalysis>('get_preview_zoom_indicators', {
        videoPath: filePath
      });

      if (!current()) return;
      set((state) => {
        state.previewZoomAnalysis = analysis;
        state.previewZoomLoading = false;
      });
    } catch (error) {
      if (!current()) return;
      console.error('Failed to load preview zoom data:', error);

      set((state) => {
        state.previewZoomAnalysis = null;
        state.previewZoomLoading = false;
      });
    }
  },

  loadMouseEvents: async (sidecarPath: string) => {
    const current = beginLoad('loadMouseEvents');
    set((state) => { state.mouseEventsLoading = true; });

    try {
      const sidecarContent = await invoke<string>('read_sidecar_file', { path: sidecarPath });
      if (!current()) return;
      const rawData = JSON.parse(sidecarContent);

      let rawEvents: any[];
      let displayWidth = 1920;
      let displayHeight = 1080;
      let scaleFactor = 1.0;
      let recordingArea: { x: number; y: number; width: number; height: number } | null = null;
      let screenWidth: number | null = null;
      let screenHeight: number | null = null;

      let captureMode: 'display' | 'window' | null = null;

      if (Array.isArray(rawData)) {
        rawEvents = rawData;
      } else if (rawData.mouse_events) {
        rawEvents = rawData.mouse_events;
        displayWidth = rawData.display_width || 1920;
        displayHeight = rawData.display_height || 1080;
        scaleFactor = rawData.scale_factor || 1.0;
        recordingArea = rawData.recording_area || null;
        captureMode = rawData.capture_mode || null;
        screenWidth = rawData.screen_width ?? null;
        screenHeight = rawData.screen_height ?? null;
      } else {
        throw new Error('Invalid sidecar format');
      }

      const data: MouseEventData[] = rawEvents.map((e: any) => {
        const event = e.base || e;
        let eventTypeStr = 'Move';
        if (typeof event.event_type === 'object' && event.event_type !== null) {
          eventTypeStr = Object.keys(event.event_type)[0] || 'Move';
        } else if (typeof event.event_type === 'string') {
          eventTypeStr = event.event_type;
        }
        return {
          base: {
            timestamp: event.timestamp,
            x: event.x,
            y: event.y,
            event_type: eventTypeStr,
            display_id: event.display_id || null,
          },
          window_id: e.window_id || null,
          app_name: e.app_name || null,
          is_double_click: e.is_double_click || false,
          cluster_id: e.cluster_id || null,
        };
      }).sort((a, b) => a.base.timestamp - b.base.timestamp);

      if (!current()) return;
      set((state) => {
        state.mouseEvents = data;
        state.typingRanges = typingRanges(rawData.key_events);
        state.shortcutIntervals = shortcutsFromSidecar(rawData.key_events);
        state.mouseEventsLoading = false;
        state.scaleFactor = scaleFactor;
        state.recordingArea = recordingArea;
        state.captureMode = captureMode;

        if (captureMode === 'window' && state.videoWidth && state.videoHeight) {
          // Window recordings: displayResolution = window dims (used for cursor normalization).
          // screenResolution = host display dims (used for proportional sizing on the canvas).
          state.displayResolution = { width: state.videoWidth, height: state.videoHeight };
          state.screenResolution = (screenWidth && screenHeight)
            ? { width: screenWidth, height: screenHeight }
            : null;
          // Loading input metadata must preserve the user's saved layout choices.
        } else {
          state.displayResolution = { width: displayWidth, height: displayHeight };
          state.screenResolution = null;
        }
      });
    } catch (error) {
      if (!current()) return;
      console.error('[MOUSE] Failed to load mouse events:', error);

      set((state) => {
        state.mouseEvents = null;
        state.typingRanges = [];
        state.shortcutIntervals = [];
        state.mouseEventsLoading = false;
      });
    }
  },

  getCursorAtTime: (time: number) => {
    const state = get();
    if (!state.mouseEvents || state.mouseEvents.length === 0) {
      return null;
    }

    const events = state.mouseEvents;
    const resolution = state.displayResolution || { width: 1920, height: 1080 };
    const effectiveX = state.recordingArea?.x ?? 0;
    const effectiveY = state.recordingArea?.y ?? 0;
    const effectiveWidth = state.recordingArea?.width ?? resolution.width;
    const effectiveHeight = state.recordingArea?.height ?? resolution.height;

    const normalize = (event: MouseEventData) => ({
      x: Math.max(0, Math.min(1, (event.base.x - effectiveX) / effectiveWidth)),
      y: Math.max(0, Math.min(1, (event.base.y - effectiveY) / effectiveHeight)),
    });

    let left = 0;
    let right = events.length;

    while (left < right) {
      const mid = Math.floor((left + right) / 2);
      if (events[mid].base.timestamp <= time) {
        left = mid + 1;
      } else {
        right = mid;
      }
    }

    const next = events[left];
    const previous = events[left - 1];

    if (!previous) {
      return normalize(next);
    }
    if (!next) {
      return normalize(previous);
    }

    const previousPos = normalize(previous);
    const nextPos = normalize(next);
    const span = next.base.timestamp - previous.base.timestamp;
    const t = span > 0 ? Math.max(0, Math.min(1, (time - previous.base.timestamp) / span)) : 0;

    return {
      x: previousPos.x + (nextPos.x - previousPos.x) * t,
      y: previousPos.y + (nextPos.y - previousPos.y) * t,
    };
  },
});
};
