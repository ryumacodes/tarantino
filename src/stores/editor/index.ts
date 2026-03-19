// Editor Store - Re-exports

// Types
export type {
  ZoomKeyframe,
  TimelineClip,
  TimelineTrack,
  TimelineTool,
  TimelineSelection,
  SnappingTarget,
  TimelineViewState,
  WebcamKeyframe,
  Overlay,
  AudioSettings,
  ExportResolution,
  ExportFrameRate,
  ExportQuality,
  ExportFormat,
  ExportSettings,
  BackgroundType,
  GradientDirection,
  AspectRatio,
  DeviceFrame,
  GradientStop,
  VisualSettings,
  ZoomConfig,
  ZoomBlock,
  ZoomAnalysis,
  PreviewZoomIndicator,
  PreviewZoomAnalysis,
  MouseEventData,
  EditorState,
  EditorActions,
} from './types';

// Constants
export {
  DEFAULT_EXPORT_SETTINGS,
  RESOLUTION_DIMENSIONS,
  ASPECT_RATIOS,
  DEVICE_FRAMES,
  WALLPAPERS,
  SPRING_PRESETS,
  ZOOM_SPEED_PRESETS,
  DEFAULT_VISUAL_SETTINGS,
} from './constants';

export type { SpringPreset } from './constants';

// Store
export { useEditorStore } from './store';
