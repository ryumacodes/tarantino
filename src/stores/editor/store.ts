// Editor Store Implementation - Main store composing slices

import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { invoke } from '@tauri-apps/api/core';

import type {
  EditorState,
  EditorActions,
  ZoomKeyframe,
  WebcamKeyframe,
  Overlay,
  ZoomBlock,
  ZoomAnalysis,
  PreviewZoomAnalysis,
  MouseEventData,
  TimelineTool,
  TimelineTrack,
  TimelineClip,
  SnappingTarget,
} from './types';

import {
  DEFAULT_EXPORT_SETTINGS,
  DEFAULT_VISUAL_SETTINGS,
  RESOLUTION_DIMENSIONS,
  WALLPAPERS,
} from './constants';

// Import slice action implementations
import { createPlaybackActions } from './actions/playbackActions';
import { createZoomActions } from './actions/zoomActions';
import { createClipsActions } from './actions/clipsActions';
import { createSettingsActions } from './actions/settingsActions';

// Initial state
const initialState: EditorState = {
  videoFilePath: null,
  projectTitle: 'Untitled Recording',
  recordedAt: null,
  duration: 30000,
  currentTime: 0,
  trimStart: 0,
  trimEnd: 30000,
  zoomKeyframes: [],
  webcamKeyframes: [],
  overlays: [],
  audioSettings: {
    micGain: 0,
    systemGain: 0,
    noiseGate: false,
    dualTrack: false
  },
  selectedKeyframe: null,
  thumbnails: [],
  thumbnailsLoading: false,
  history: [],
  historyIndex: -1,
  hasWebcam: false,
  hasMicrophone: false,
  hasSystemAudio: false,
  zoomAnalysis: null,
  zoomLoading: false,
  selectedBlockId: null,
  previewZoomAnalysis: null,
  previewZoomLoading: false,
  mouseEvents: null,
  mouseEventsLoading: false,
  displayResolution: null,
  scaleFactor: 1.0,
  recordingArea: null,
  isPlaying: false,
  tracks: [],
  clips: [],
  currentTool: 'select' as TimelineTool,
  selection: {
    clipIds: [],
    trackIds: [],
    keyframeIds: []
  },
  snappingEnabled: true,
  snappingTargets: [],
  viewState: {
    zoom: 1,
    scrollPosition: 0,
    playheadFollowing: true
  },
  visualSettings: DEFAULT_VISUAL_SETTINGS,
  exportSettings: DEFAULT_EXPORT_SETTINGS,
};

export const useEditorStore = create<EditorState & EditorActions>()(
  devtools(
    immer((set, get) => ({
      ...initialState,

      get canUndo() {
        return get().historyIndex > 0;
      },

      get canRedo() {
        return get().historyIndex < get().history.length - 1;
      },

      // Core initialization
      initializeEditor: (filePath, duration, hasWebcam = false, hasMicrophone = false, hasSystemAudio = false) => {
        set((state) => {
          console.log('Editor store: Initializing with filePath:', filePath, 'duration:', duration);
          state.videoFilePath = filePath;
          const fileName = filePath.split('/').pop() || 'recording';
          const nameWithoutExt = fileName.replace(/\.[^/.]+$/, '');
          state.projectTitle = nameWithoutExt;
          state.recordedAt = Date.now();
          state.duration = duration;
          state.trimStart = 0;
          state.trimEnd = duration;
          state.currentTime = 0;
          state.hasWebcam = hasWebcam;
          state.hasMicrophone = hasMicrophone;
          state.hasSystemAudio = hasSystemAudio;
          state.zoomKeyframes = [];
          state.webcamKeyframes = [];
          state.overlays = [];
          state.selectedKeyframe = null;
          state.thumbnails = [];
          state.thumbnailsLoading = false;
          state.history = [];
          state.historyIndex = -1;
          state.zoomAnalysis = null;
          state.zoomLoading = false;
          state.selectedBlockId = null;
          state.previewZoomAnalysis = null;
          state.previewZoomLoading = false;

          // Initialize main video track and clip
          const videoTrackId = 'video-track-main';
          const videoClipId = crypto.randomUUID();

          const videoTrack: TimelineTrack = {
            id: videoTrackId,
            name: 'Video',
            type: 'video',
            clips: [],
            height: 80,
            visible: true,
            muted: false,
            solo: false,
            locked: false,
            order: 0,
          };

          const videoClip: TimelineClip = {
            id: videoClipId,
            name: filePath.split('/').pop() || 'Main Video',
            type: 'video',
            trackId: videoTrackId,
            startTime: 0,
            duration: duration,
            sourceIn: 0,
            sourceOut: duration,
            sourceFilePath: filePath,
            enabled: true,
            locked: false,
            color: '#6366f1',
            playbackRate: 1.0,
          };

          state.tracks = [videoTrack];
          state.clips = [videoClip];
          videoTrack.clips = [videoClip];
        });

        get().loadThumbnails(filePath);
        get().loadPreviewZoomData(filePath);
        get().loadZoomData(filePath);
      },

      loadThumbnails: async (filePath) => {
        set((state) => { state.thumbnailsLoading = true; });
        try {
          const durationSec = get().duration / 1000;
          const count = Math.max(5, Math.min(50, Math.ceil(durationSec * 1.0)));
          const thumbnailPaths = await invoke<string[]>('extract_video_thumbnails', {
            videoPath: filePath,
            thumbnailCount: count,
            thumbnailWidth: 160
          });
          set((state) => {
            state.thumbnails = thumbnailPaths;
            state.thumbnailsLoading = false;
          });
        } catch (error) {
          console.error('Failed to load thumbnails:', error);
          set((state) => { state.thumbnailsLoading = false; });
        }
      },

      // Playback actions
      ...createPlaybackActions(set, get),

      // Zoom actions
      ...createZoomActions(set, get),

      // Clips actions
      ...createClipsActions(set, get),

      // Settings actions
      ...createSettingsActions(set, get),
    }))
  )
);
