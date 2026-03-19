// Zoom Actions
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

export const createZoomActions = (set: SetFn, get: GetFn) => ({
  // Zoom Keyframe Actions
  addZoomKeyframe: (keyframe: ZoomKeyframe) => set((state) => {
    state.zoomKeyframes.push(keyframe);
    state.selectedKeyframe = keyframe;
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  updateZoomKeyframe: (id: string, updates: Partial<ZoomKeyframe>) => set((state) => {
    const index = state.zoomKeyframes.findIndex(kf => kf.id === id);
    if (index !== -1) {
      Object.assign(state.zoomKeyframes[index], updates);
      state.history.push({ ...state });
      state.historyIndex++;
    }
  }),

  deleteZoomKeyframe: (id: string) => set((state) => {
    state.zoomKeyframes = state.zoomKeyframes.filter(kf => kf.id !== id);
    if (state.selectedKeyframe?.id === id) {
      state.selectedKeyframe = null;
    }
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  // Webcam Keyframe Actions
  addWebcamKeyframe: (keyframe: WebcamKeyframe) => set((state) => {
    state.webcamKeyframes.push(keyframe);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  updateWebcamKeyframe: (id: string, updates: Partial<WebcamKeyframe>) => set((state) => {
    const index = state.webcamKeyframes.findIndex(kf => kf.id === id);
    if (index !== -1) {
      Object.assign(state.webcamKeyframes[index], updates);
      state.history.push({ ...state });
      state.historyIndex++;
    }
  }),

  deleteWebcamKeyframe: (id: string) => set((state) => {
    state.webcamKeyframes = state.webcamKeyframes.filter(kf => kf.id !== id);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  // Zoom Block Actions
  loadZoomData: async (filePath: string) => {
    console.log('[ZOOM] Loading zoom data for:', filePath);
    const expectedPath = filePath.replace('.mp4', '') + '.auto_zoom.json';
    console.log('[ZOOM] Expected auto_zoom.json path:', expectedPath);
    set((state) => { state.zoomLoading = true; });

    try {
      const analysis = await invoke<ZoomAnalysis | null>('load_auto_zoom_data', {
        videoPath: filePath
      });

      console.log('[ZOOM] Raw response from load_auto_zoom_data:', analysis);

      if (!analysis) {
        console.warn('[ZOOM] No zoom analysis data returned - file may not exist');
      } else {
        console.log('[ZOOM] Zoom analysis loaded:');
        console.log('  - total_clicks:', analysis.total_clicks);
        console.log('  - zoom_blocks count:', analysis.zoom_blocks?.length ?? 0);
        console.log('  - session_duration:', analysis.session_duration);
        if (analysis.zoom_blocks) {
          analysis.zoom_blocks.forEach((block, i) => {
            console.log(`  - Block ${i}: ${block.start_time}ms-${block.end_time}ms, ${block.zoom_factor}x at (${block.center_x.toFixed(2)}, ${block.center_y.toFixed(2)})`);
          });
        }
      }

      set((state) => {
        if (analysis && analysis.zoom_blocks) {
          const videoDuration = state.duration;
          const originalCount = analysis.zoom_blocks.length;
          analysis.zoom_blocks = analysis.zoom_blocks
            .map(block => ({
              ...block,
              end_time: Math.min(block.end_time, videoDuration),
            }))
            .filter(block => block.end_time > block.start_time + 500);
          const filteredCount = analysis.zoom_blocks.length;
          if (originalCount !== filteredCount) {
            console.log(`[ZOOM] Filtered ${originalCount - filteredCount} blocks (too short after duration clamp)`);
          }
          analysis.session_duration = videoDuration;
        }
        state.zoomAnalysis = analysis;
        state.zoomLoading = false;
      });
    } catch (error) {
      console.error('[ZOOM] Failed to load zoom data:', error);
      set((state) => {
        state.zoomAnalysis = null;
        state.zoomLoading = false;
      });
    }
  },

  updateZoomBlock: (blockId: string, updates: Partial<ZoomBlock>) => set((state) => {
    if (!state.zoomAnalysis) return;

    const blockIndex = state.zoomAnalysis.zoom_blocks.findIndex(block => block.id === blockId);
    if (blockIndex === -1) return;

    const clampedUpdates = {
      ...updates,
      ...(updates.end_time !== undefined && {
        end_time: Math.min(updates.end_time, state.duration),
      }),
    };
    Object.assign(state.zoomAnalysis.zoom_blocks[blockIndex], clampedUpdates);

    if (updates.center_x !== undefined || updates.center_y !== undefined) {
      state.zoomAnalysis.zoom_blocks[blockIndex].is_manual = true;
    }

    console.log(`Updated zoom block ${blockId}:`, clampedUpdates);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  deleteZoomBlock: (blockId: string) => set((state) => {
    if (!state.zoomAnalysis) return;

    state.zoomAnalysis.zoom_blocks = state.zoomAnalysis.zoom_blocks.filter(
      block => block.id !== blockId
    );

    if (state.selectedBlockId === blockId) {
      state.selectedBlockId = null;
    }

    console.log(`Deleted zoom block ${blockId}`);
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  addZoomBlock: (block: ZoomBlock) => set((state) => {
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
    console.log(`Added zoom block ${clampedBlock.id}`);
    state.history.push({ ...state });
    state.historyIndex++;
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
      console.log('Zoom data saved successfully');
    } catch (error) {
      console.error('Failed to save zoom data:', error);
      throw error;
    }
  },

  setSelectedBlockId: (id: string | null) => set((state) => {
    state.selectedBlockId = id;
  }),

  // Preview Zoom Actions
  loadPreviewZoomData: async (filePath: string) => {
    console.log('Loading preview zoom data for:', filePath);
    set((state) => { state.previewZoomLoading = true; });

    try {
      const analysis = await invoke<PreviewZoomAnalysis>('get_preview_zoom_indicators', {
        videoPath: filePath
      });

      console.log('Preview zoom data loaded:', analysis);
      set((state) => {
        state.previewZoomAnalysis = analysis;
        state.previewZoomLoading = false;
      });
    } catch (error) {
      console.error('Failed to load preview zoom data:', error);
      set((state) => {
        state.previewZoomAnalysis = null;
        state.previewZoomLoading = false;
      });
    }
  },

  // Mouse Event Actions
  loadMouseEvents: async (sidecarPath: string) => {
    console.log('[MOUSE] Loading mouse events from:', sidecarPath);
    set((state) => { state.mouseEventsLoading = true; });

    try {
      console.log('[MOUSE] Invoking read_sidecar_file...');
      const sidecarContent = await invoke<string>('read_sidecar_file', { path: sidecarPath });
      console.log('[MOUSE] Sidecar file content length:', sidecarContent?.length ?? 0);
      const rawData = JSON.parse(sidecarContent);
      console.log('[MOUSE] Parsed sidecar data type:', Array.isArray(rawData) ? 'array' : typeof rawData);

      let rawEvents: any[];
      let displayWidth = 1920;
      let displayHeight = 1080;
      let scaleFactor = 1.0;
      let recordingArea: { x: number; y: number; width: number; height: number } | null = null;

      if (Array.isArray(rawData)) {
        rawEvents = rawData;
      } else if (rawData.mouse_events) {
        rawEvents = rawData.mouse_events;
        displayWidth = rawData.display_width || 1920;
        displayHeight = rawData.display_height || 1080;
        scaleFactor = rawData.scale_factor || 1.0;
        recordingArea = rawData.recording_area || null;
        console.log('Display dimensions from sidecar:', displayWidth, 'x', displayHeight);
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
      });

      console.log('[MOUSE] Mouse events loaded:', data.length, 'events');
      const clickCount = data.filter(e => e.base.event_type === 'ButtonPress').length;
      console.log('[MOUSE] Click events (ButtonPress):', clickCount);
      console.log('[MOUSE] Display resolution:', displayWidth, 'x', displayHeight, 'scale:', scaleFactor);
      if (recordingArea) {
        console.log('[MOUSE] Recording area:', recordingArea);
      }
      set((state) => {
        state.mouseEvents = data;
        state.mouseEventsLoading = false;
        state.displayResolution = { width: displayWidth, height: displayHeight };
        state.scaleFactor = scaleFactor;
        state.recordingArea = recordingArea;
      });
    } catch (error) {
      console.error('[MOUSE] Failed to load mouse events:', error);
      console.error('[MOUSE] This may indicate the .mouse.json file does not exist');
      set((state) => {
        state.mouseEvents = null;
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
    let left = 0;
    let right = events.length - 1;

    while (left < right) {
      const mid = Math.floor((left + right + 1) / 2);
      if (events[mid].base.timestamp <= time) {
        left = mid;
      } else {
        right = mid - 1;
      }
    }

    const event = events[left];
    if (!event) return null;

    const resolution = state.displayResolution || { width: 1920, height: 1080 };
    const effectiveX = state.recordingArea?.x ?? 0;
    const effectiveY = state.recordingArea?.y ?? 0;
    const effectiveWidth = state.recordingArea?.width ?? resolution.width;
    const effectiveHeight = state.recordingArea?.height ?? resolution.height;

    const normalizedX = (event.base.x - effectiveX) / effectiveWidth;
    const normalizedY = (event.base.y - effectiveY) / effectiveHeight;

    return {
      x: Math.max(0, Math.min(1, normalizedX)),
      y: Math.max(0, Math.min(1, normalizedY))
    };
  },
});
