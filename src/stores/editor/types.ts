// Editor Store Types

export interface ZoomKeyframe {
  id: string;
  time: number;
  centerX: number;
  centerY: number;
  scale: number;
  duration: number;
  easing: string;
}

// Timeline Clip System
export interface TimelineClip {
  id: string;
  name: string;
  type: 'video' | 'audio' | 'webcam' | 'screen';
  trackId: string;
  startTime: number; // Timeline position in ms
  duration: number; // Clip duration in ms (adjusted for speed)
  sourceIn: number; // In point in source media
  sourceOut: number; // Out point in source media
  sourceFilePath?: string;
  enabled: boolean;
  locked: boolean;
  color?: string;
  playbackRate: number; // Playback speed multiplier (1.0 = normal, 2.0 = 2x, 0.5 = half speed)
  metadata?: Record<string, any>;
}

export interface TimelineTrack {
  id: string;
  name: string;
  type: 'video' | 'audio' | 'webcam' | 'screen';
  clips: TimelineClip[];
  height: number;
  visible: boolean;
  muted: boolean;
  solo: boolean;
  locked: boolean;
  order: number;
}

export type TimelineTool = 'select' | 'scissors' | 'trim' | 'slip' | 'slide' | 'pan' | 'zoom';

export interface TimelineSelection {
  clipIds: string[];
  trackIds: string[];
  keyframeIds: string[];
}

export interface SnappingTarget {
  time: number;
  type: 'clip-start' | 'clip-end' | 'keyframe' | 'marker' | 'playhead';
  id: string;
}

export interface TimelineViewState {
  zoom: number;
  scrollPosition: number;
  playheadFollowing: boolean;
}

export interface WebcamKeyframe {
  id: string;
  time: number;
  x: number;
  y: number;
  size: number;
  shape: 'circle' | 'roundrect';
  visible: boolean;
}

export interface Overlay {
  id: string;
  type: 'ring' | 'arrow' | 'text' | 'lower-third';
  startTime: number;
  endTime: number;
  x: number;
  y: number;
  properties: Record<string, any>;
}

export interface AudioSettings {
  micGain: number;
  systemGain: number;
  noiseGate: boolean;
  dualTrack: boolean;
}

// Export Settings
export type ExportResolution = '720p' | '1080p' | '1440p' | '4k' | 'custom';
export type ExportFrameRate = 24 | 30 | 60;
export type ExportQuality = 'low' | 'medium' | 'high';
export type ExportFormat = 'mp4' | 'mov' | 'webm' | 'gif';

export interface ExportSettings {
  resolution: ExportResolution;
  customWidth?: number;
  customHeight?: number;
  frameRate: ExportFrameRate;
  quality: ExportQuality;
  format: ExportFormat;
}

// Visual Settings (Screen Studio style)
export type BackgroundType = 'solid' | 'gradient' | 'wallpaper' | 'image';
export type GradientDirection = 'to-right' | 'to-bottom' | 'to-bottom-right' | 'radial';
export type AspectRatio = '16:9' | '9:16' | '4:3' | '1:1' | '21:9' | 'auto';

// Device frame types for mockups
export type DeviceFrame = 'none' | 'iphone-15-pro' | 'iphone-15' | 'ipad-pro' | 'macbook-pro' | 'browser';

export interface GradientStop {
  color: string;
  position: number; // 0-100
}

export interface VisualSettings {
  // Background
  backgroundType: BackgroundType;
  backgroundColor: string;
  gradientStops: GradientStop[];
  gradientDirection: GradientDirection;
  wallpaperId: string | null; // Predefined wallpaper ID
  customBackgroundImage: string | null; // User uploaded image path

  // Frame
  padding: number; // 0-100 (percentage of video size)
  cornerRadius: number; // 0-50 pixels
  shadowEnabled: boolean;
  shadowIntensity: number; // 0-100
  shadowBlur: number; // 0-100
  shadowOffsetX: number; // -50 to 50
  shadowOffsetY: number; // -50 to 50
  inset: number; // 0-20 (inner padding)

  // Cursor
  cursorScale: number; // 0.5-3.0
  cursorSmoothing: number; // 0-1 (smoothness factor)
  hideCursorWhenIdle: boolean;
  idleTimeout: number; // ms before hiding cursor

  // Cursor Customization
  cursorColor: string; // Main cursor color (default white)
  cursorHighlightColor: string; // Color for click highlights
  cursorRippleColor: string; // Color for the ripple effect
  cursorShadowIntensity: number; // Shadow intensity 0-100
  cursorTrailEnabled: boolean; // Enable cursor trail effect
  cursorTrailLength: number; // Number of trail positions (5-30)
  cursorTrailOpacity: number; // Trail opacity 0-1

  // Cursor Style & Behavior
  cursorStyle: 'pointer' | 'circle' | 'filled' | 'outline' | 'dotted';
  alwaysUsePointer: boolean; // Don't change cursor style even when selecting text
  hideCursor: boolean; // Completely hide cursor
  loopCursorPosition: boolean; // Return cursor to start position at video end

  // Click Effects
  clickEffect: 'none' | 'circle' | 'ripple';

  // Cursor Rotation
  cursorRotation: number; // 0-360 degrees static rotation
  rotateCursorWhileMoving: boolean; // Dynamic rotation based on horizontal movement
  rotationIntensity: number; // 0-100 intensity for dynamic rotation

  // Advanced Cursor Options
  stopCursorAtEnd: boolean; // Freeze cursor before video end
  stopCursorDuration: number; // ms before end to stop cursor
  removeCursorShakes: boolean; // Filter micro-movements
  shakesThreshold: number; // Pixel threshold for shake detection
  optimizeCursorChanges: boolean; // Minimize rapid cursor type changes

  // Animation (Spring Physics - Screen Studio style)
  zoomSpeedPreset: 'slow' | 'mellow' | 'quick' | 'rapid';
  cursorSpeedPreset: 'slow' | 'mellow' | 'quick' | 'rapid'; // Separate from zoom

  // Motion Blur (3 independent channels, Screen Studio style)
  motionBlurEnabled: boolean;
  motionBlurPanIntensity: number; // 0-100, blur during camera pans
  motionBlurZoomIntensity: number; // 0-100, blur during zoom in/out
  motionBlurCursorIntensity: number; // 0-100, blur on cursor movement

  // Output
  aspectRatio: AspectRatio;

  // Device Frame
  deviceFrame: DeviceFrame;
  deviceFrameColor: string; // Frame color (e.g., 'black', 'silver', 'gold')
}

// Simplified zoom types (matching new Rust backend)
export interface ZoomConfig {
  enabled: boolean;
  zoom_factor: number;
  zoom_duration: number;
  min_click_spacing: number;
}

export interface ZoomCenter {
  x: number;               // Normalized center X (0-1)
  y: number;               // Normalized center Y (0-1)
  time: number;            // When to pan to this center (ms)
}

export interface ZoomBlock {
  id: string;
  click_x: number;         // First click position X
  click_y: number;         // First click position Y
  center_x: number;        // Initial zoom center X
  center_y: number;        // Initial zoom center Y
  start_time: number;      // When zoom starts
  end_time: number;        // When zoom ends
  zoom_factor: number;     // Zoom level (default from config)
  is_manual: boolean;      // True if user manually adjusted the zoom area
  centers?: ZoomCenter[];  // Re-center points from merged clicks
  kind?: 'click' | 'typing';
  zoom_in_speed?: 'slow' | 'mellow' | 'quick' | 'rapid';   // per-block, falls back to global zoomSpeedPreset
  zoom_out_speed?: 'slow' | 'mellow' | 'quick' | 'rapid';  // per-block, falls back to global zoomSpeedPreset
}

export interface ZoomAnalysis {
  zoom_blocks: ZoomBlock[];
  total_clicks: number;
  session_duration: number;
  config: ZoomConfig;
}

// Preview zoom indicators (shown before full processing)
export interface PreviewZoomIndicator {
  id: string;
  click_time: number;       // When the click occurred (ms)
  click_x: number;          // Click position X (0-1 normalized)
  click_y: number;          // Click position Y (0-1 normalized)
  preview_start: number;    // Preview zoom start time (ms)
  preview_end: number;      // Preview zoom end time (ms)
  confidence: number;       // Confidence this will become a zoom (0-1)
}

export interface PreviewZoomAnalysis {
  indicators: PreviewZoomIndicator[];
  total_clicks: number;
  total_indicators: number;
  session_duration: number;
  analysis_time_ms: number;
}

// Mouse event data from .mouse.json sidecar
export interface MouseEventData {
  base: {
    timestamp: number;
    x: number;
    y: number;
    event_type: string;
    display_id: string | null;
  };
  window_id: number | null;
  app_name: string | null;
  is_double_click: boolean;
  cluster_id: string | null;
}

export interface EditorState {
  videoFilePath: string | null;
  projectTitle: string;
  recordedAt: number | null; // Timestamp in milliseconds
  duration: number;
  currentTime: number;
  trimStart: number;
  trimEnd: number;
  zoomKeyframes: ZoomKeyframe[];
  webcamKeyframes: WebcamKeyframe[];
  overlays: Overlay[];
  audioSettings: AudioSettings;
  selectedKeyframe: ZoomKeyframe | null;
  thumbnails: string[];
  thumbnailsLoading: boolean;
  history: any[];
  historyIndex: number;
  hasWebcam: boolean;
  hasMicrophone: boolean;
  hasSystemAudio: boolean;
  isPlaying: boolean;
  // Zoom data
  zoomAnalysis: ZoomAnalysis | null;
  zoomLoading: boolean;
  // Selected zoom block (for inline editing in properties panel)
  selectedBlockId: string | null;
  // Preview zoom data
  previewZoomAnalysis: PreviewZoomAnalysis | null;
  previewZoomLoading: boolean;
  // Mouse events for cursor panning
  mouseEvents: MouseEventData[] | null;
  mouseEventsLoading: boolean;
  displayResolution: { width: number; height: number } | null;
  scaleFactor: number;
  recordingArea: { x: number; y: number; width: number; height: number } | null;
  // Timeline System
  tracks: TimelineTrack[];
  clips: TimelineClip[];
  currentTool: TimelineTool;
  selection: TimelineSelection;
  snappingEnabled: boolean;
  snappingTargets: SnappingTarget[];
  viewState: TimelineViewState;
  // Visual Settings (Screen Studio style)
  visualSettings: VisualSettings;
  // Export Settings
  exportSettings: ExportSettings;
}

export interface EditorActions {
  initializeEditor: (filePath: string, duration: number, hasWebcam?: boolean, hasMicrophone?: boolean, hasSystemAudio?: boolean) => void;
  setProjectTitle: (title: string) => void;
  loadThumbnails: (filePath: string) => Promise<void>;
  setIsPlaying: (isPlaying: boolean) => void;
  setCurrentTime: (time: number) => void;
  setDuration: (duration: number) => void;
  setTrimStart: (time: number) => void;
  setTrimEnd: (time: number) => void;
  addZoomKeyframe: (keyframe: ZoomKeyframe) => void;
  updateZoomKeyframe: (id: string, updates: Partial<ZoomKeyframe>) => void;
  deleteZoomKeyframe: (id: string) => void;
  addWebcamKeyframe: (keyframe: WebcamKeyframe) => void;
  updateWebcamKeyframe: (id: string, updates: Partial<WebcamKeyframe>) => void;
  deleteWebcamKeyframe: (id: string) => void;
  addOverlay: (overlay: Overlay) => void;
  updateOverlay: (id: string, updates: Partial<Overlay>) => void;
  deleteOverlay: (id: string) => void;
  updateAudioSettings: (settings: Partial<AudioSettings>) => void;
  selectKeyframe: (keyframe: ZoomKeyframe | null) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  // Zoom actions
  loadZoomData: (filePath: string) => Promise<void>;
  updateZoomBlock: (blockId: string, updates: Partial<ZoomBlock>) => void;
  deleteZoomBlock: (blockId: string) => void;
  addZoomBlock: (block: ZoomBlock) => void;
  saveZoomData: () => Promise<void>;
  setSelectedBlockId: (id: string | null) => void;
  // Preview zoom actions
  loadPreviewZoomData: (filePath: string) => Promise<void>;
  // Mouse event actions
  loadMouseEvents: (sidecarPath: string) => Promise<void>;
  getCursorAtTime: (time: number) => { x: number; y: number } | null;
  // Timeline actions
  setCurrentTool: (tool: TimelineTool) => void;
  addTrack: (track: TimelineTrack) => void;
  updateTrack: (id: string, updates: Partial<TimelineTrack>) => void;
  deleteTrack: (id: string) => void;
  addClip: (clip: TimelineClip) => void;
  updateClip: (id: string, updates: Partial<TimelineClip>) => void;
  deleteClip: (id: string) => void;
  cutClip: (clipId: string, time: number) => void;
  cutClipsAtTime: (time: number, trackId?: string) => void;
  getClipsAtTime: (time: number, trackId?: string) => TimelineClip[];
  setClipPlaybackRate: (clipId: string, rate: number) => void;
  moveClip: (clipId: string, newStartTime: number, newTrackId?: string) => void;
  selectClips: (clipIds: string[], addToSelection?: boolean) => void;
  clearSelection: () => void;
  setSnappingEnabled: (enabled: boolean) => void;
  updateSnappingTargets: () => void;
  setViewState: (updates: Partial<TimelineViewState>) => void;
  // Visual settings actions
  updateVisualSettings: (settings: Partial<VisualSettings>) => void;
  resetVisualSettings: () => void;
  applyWallpaper: (wallpaperId: string) => void;
  // Export settings actions
  updateExportSettings: (settings: Partial<ExportSettings>) => void;
  resetExportSettings: () => void;
  getExportDimensions: () => { width: number; height: number };
}
