import {parseCaptionSettings} from "../../utils/captions/settings";
import { parseShortcutSettings } from "../../utils/shortcuts/settings";
import { createThumbnailActions } from './actions/thumbnailActions';
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
  recordingRevision: 0,
  videoFilePath: null,
  projectTitle: 'Untitled Recording',
  recordedAt: null,
  duration: 30000,
  currentTime: 0,
  playbackRevision: 0,
  trimStart: 0,
  trimEnd: 30000,
  zoomKeyframes: [],
  webcamKeyframes: [],
  overlays: [],
  audioSettings: {
    micMuted: false,
    systemMuted: false,
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
  historyGroup: null,
  hasWebcam: false,
  hasMicrophone: false,
  hasSystemAudio: false,
  zoomAnalysis: null,
  zoomLoading: false,
  selectedBlockId: null,
  previewZoomAnalysis: null,
  previewZoomLoading: false,
  mouseEvents: null,
  shortcutSettings: parseShortcutSettings(undefined),
  shortcutIntervals: [],
  masks: [],
  transcripts: [],
  captionSettings: parseCaptionSettings(undefined),
  typingRanges: [],
  dismissedTypingRanges: [],
  mouseEventsLoading: false,
  videoWidth: null,
  videoHeight: null,
  displayResolution: null,
  screenResolution: null,
  scaleFactor: 1.0,
  recordingArea: null,
  captureMode: null,
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

      canUndo: false,
      canRedo: false,

      initializeEditor: (filePath, duration, hasWebcam = false, hasMicrophone = false, hasSystemAudio = false, videoWidth = null, videoHeight = null) => {
        set((state) => {
          state.audioSettings.backgroundAudio = undefined;
          state.recordingRevision = (state.recordingRevision ?? 0)+1;
          state.isPlaying = false;
          state.playbackRevision = state.playbackRevision+1;
          state.mouseEvents = null;
          state.typingRanges = [];
          state.shortcutIntervals = [];
          state.masks = [];
          state.transcripts = [];
          state.captionSettings = parseCaptionSettings(undefined);
          state.shortcutSettings = parseShortcutSettings(undefined);
          state.dismissedTypingRanges = [];
          state.mouseEventsLoading = false;
          state.displayResolution = null;
          state.screenResolution = null;
          state.recordingArea = null;
          state.captureMode = null;
          state.scaleFactor = 1;
          state.selection = {clipIds:[],trackIds:[],keyframeIds:[]};
          state.videoFilePath = filePath;
          state.videoWidth = videoWidth;
          state.videoHeight = videoHeight;
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
          state.historyGroup = null;
          state.canUndo = false;
          state.canRedo = false;
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

      ...createThumbnailActions(set, get),

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
