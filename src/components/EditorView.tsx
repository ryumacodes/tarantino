import { isMacRuntime } from '../utils/platform';
import {maskExportPayload} from "../utils/masks/export";
import {captionExportPayload} from '../utils/captions/exportRaster';
import {shortcutExportPayload} from "../utils/shortcuts/exportRaster";
import { useShallow } from 'zustand/react/shallow';
import { selectEditorShellInputs } from './editor/shellInputs';
import { loadCameraSilence } from '../utils/camera/silence';
import BackgroundAudioDrop from './editor/BackgroundAudioDrop';
import BackgroundAudioPreview from './editor/preview/BackgroundAudioPreview';
import { exportBackgroundAudio } from '../utils/audio/backgroundAudio';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import EditorTopBar from './editor/EditorTopBar';
import VideoPreviewPanel from './editor/VideoPreviewPanel';
import PropertiesPanel from './editor/PropertiesPanel';
import ProfessionalTimeline from './editor/ProfessionalTimeline';
import { EditorShortcutOverlays, type EditorShortcutAction } from './editor/EditorShortcutOverlays';
import { useEditorStore } from '../stores/editor';
import '../styles/dracula-theme.css';
import { timelineDuration } from '../utils/timeline/previewTimeline';
import { exportTimeline } from '../utils/timeline/exportTimeline';
import { exportZoomBlocks } from '../utils/motion/zoomTrajectory';
import { resolveScreenSpring } from '../utils/motion/screenStyle';
import { resolveCursorSpring } from '../utils/motion/cursorStyle';

interface EditorViewProps {
  onClose: () => void;
}

const EditorView: React.FC<EditorViewProps> = ({ onClose }) => {
  const [audioImportRevision,setAudioImportRevision]=useState(0);
  const [showMouseOverlay, setShowMouseOverlay] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const exportInFlight = useRef(false);
  const [canCancelExport, setCanCancelExport] = useState(false);
  const [isCancellingExport, setIsCancellingExport] = useState(false);
  const [exportStatus, setExportStatus] = useState<{ message: string; error?: boolean } | null>(null);
  const [exportProgress, setExportProgress] = useState(0);
  const [timelineCollapsed, setTimelineCollapsed] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [notesOpen, setNotesOpen] = useState(false);
  const [flags, setFlags] = useState<number[]>([]);
  const {
    setCurrentTime,
    isPlaying,
    setIsPlaying,
    videoFilePath,
    projectTitle,
    recordedAt,
    setProjectTitle
  } = useEditorStore(useShallow(selectEditorShellInputs));


  const handlePlayPause = useCallback(() => {
    const state = useEditorStore.getState();
    if (!state.isPlaying && state.currentTime >= timelineDuration(state)) {
      state.setCurrentTime(0);
    }
    state.setIsPlaying(!state.isPlaying);
  }, []);

  const handleSeek = useCallback((time: number) => {
    setCurrentTime(time);
  }, [setCurrentTime]);

  const handleExport = async () => {
    if (!videoFilePath || exportInFlight.current) return;

    exportInFlight.current = true;
    setIsPlaying(false);
    setCommandOpen(false);
    setShortcutsOpen(false);
    setNotesOpen(false);
    setIsExporting(true);
    setExportProgress(0);
    setCanCancelExport(false);
    setIsCancellingExport(false);
    setExportStatus(null);

    let unlisten: (() => void) | undefined;
    try {
      const { invoke } = await import('@tauri-apps/api/core');

      const storeState = useEditorStore.getState();
      const { trimStart, trimEnd, zoomKeyframes, zoomAnalysis, audioSettings } = storeState;
      const exportSettings = storeState.exportSettings;
      const visualSettings = storeState.visualSettings;
      const zoomBlocks = exportZoomBlocks(storeState.zoomAnalysis, visualSettings);
      const dimensions = storeState.getExportDimensions();

      const projectTitleForExport = storeState.projectTitle || 'Untitled Recording';

      const exportConfig = {
        output_path: null,
        project_title: projectTitleForExport,
        resolution: dimensions,
        frame_rate: exportSettings.format === 'gif' ? Math.min(exportSettings.frameRate, 30) : exportSettings.frameRate,
        quality: exportSettings.quality,
        format: exportSettings.format,
        codec: exportSettings.format === 'mov' ? 'prores' : 'h264',
        trim_start: trimStart,
        trim_end: trimEnd,
        ...exportTimeline(storeState),
        zoom_blocks: zoomBlocks,
        zoom_keyframes: zoomKeyframes,
        zoom_analysis: zoomAnalysis,
        visual_settings: {
          background_type: visualSettings.backgroundType,
          background_color: visualSettings.backgroundColor,
          gradient_direction: visualSettings.gradientDirection,
          gradient_stops: visualSettings.gradientStops,
          wallpaper_id: visualSettings.wallpaperId,
          custom_background_image: visualSettings.customBackgroundImage,
          window_layout_mode: visualSettings.windowLayoutMode,
          padding: visualSettings.padding,
          corner_radius: visualSettings.cornerRadius,
          shadow_enabled: visualSettings.shadowEnabled,
          shadow_intensity: visualSettings.shadowIntensity,
          shadow_blur: visualSettings.shadowBlur,
          shadow_offset_x: visualSettings.shadowOffsetX,
          shadow_offset_y: visualSettings.shadowOffsetY,
          aspect_ratio: visualSettings.aspectRatio,
          motion_blur_enabled: visualSettings.motionBlurEnabled,
          motion_blur_pan_intensity: visualSettings.motionBlurPanIntensity / 100,
          motion_blur_zoom_intensity: visualSettings.motionBlurZoomIntensity / 100,
          motion_blur_cursor_intensity: visualSettings.motionBlurCursorIntensity / 100,
          device_frame: visualSettings.deviceFrame,
          device_frame_color: visualSettings.deviceFrameColor,
        },
        cursor_settings: {
          enabled: !visualSettings.hideCursor,
          size: visualSettings.cursorScale ?? 1.0,
          highlight_clicks: true,
          smoothing: visualSettings.cursorSmoothing ?? 0.15,
          style: visualSettings.cursorStyle ?? 'pointer',
          follow_hover: isMacRuntime && (visualSettings.followHover ?? true),
          optimize_cursor_types: visualSettings.optimizeCursorTypes ?? true,
          always_use_pointer: visualSettings.alwaysUsePointer ?? false,
          color: visualSettings.cursorColor ?? '#ffffff',
          highlight_color: visualSettings.cursorHighlightColor ?? '#ff6b6b',
          ripple_color: visualSettings.cursorRippleColor ?? '#64b4ff',
          shadow_intensity: visualSettings.cursorShadowIntensity ?? 30,
          trail_enabled: visualSettings.cursorTrailEnabled ?? false,
          trail_length: visualSettings.cursorTrailLength ?? 10,
          trail_opacity: visualSettings.cursorTrailOpacity ?? 0.5,
          click_effect: visualSettings.clickEffect ?? 'ripple',
          speed_preset: visualSettings.cursorSpeedPreset ?? 'mellow',
          spring_tension: resolveCursorSpring(visualSettings).tension,
          spring_friction: resolveCursorSpring(visualSettings).friction,
          spring_mass: resolveCursorSpring(visualSettings).mass,
          rotation: visualSettings.cursorRotation ?? 0,
          rotate_while_moving: visualSettings.rotateCursorWhileMoving ?? false,
          rotation_intensity: visualSettings.rotationIntensity ?? 50,
          hide_when_idle: visualSettings.hideCursorWhenIdle ?? true,
          idle_timeout: visualSettings.idleTimeout ?? 3000,
          stop_at_end: visualSettings.stopCursorAtEnd ?? false,
          stop_duration_ms: visualSettings.stopCursorDuration ?? 0,
          shake_threshold: visualSettings.removeCursorShakes ? visualSettings.shakesThreshold : 0,
          loop_to_start: visualSettings.loopCursorPosition ?? false,
          loop_duration_ms: 500,
        },
        background_audio: exportBackgroundAudio(audioSettings.backgroundAudio),
        audio_settings: {
          mic_muted: audioSettings.micMuted ?? false,
          system_muted: audioSettings.systemMuted ?? false,
          mic_gain: audioSettings.micGain,
          system_gain: audioSettings.systemGain,
          noise_gate: audioSettings.noiseGate,
          dual_track: audioSettings.dualTrack,
        },
        source_crop: storeState.visualSettings.sourceCrop ?? null,
        source_width: storeState.videoWidth ?? null,
        source_height: storeState.videoHeight ?? null,
        capture_mode: storeState.captureMode ?? 'display',
        screen_width: storeState.screenResolution?.width ?? null,
        screen_height: storeState.screenResolution?.height ?? null,
        window_width: storeState.recordingArea?.width ?? null,
        window_height: storeState.recordingArea?.height ?? null,
        window_mask_path: storeState.videoFilePath
          ? `${storeState.videoFilePath.replace(/\.[^/.]+$/, '')}.window-mask.png`
          : null,
        screen_motion_style: visualSettings.screenMotionStyle ?? null,
        animation_speed: visualSettings.zoomSpeedPreset ?? 'mellow',
        zoom_spring_tension: resolveScreenSpring(visualSettings).tension,
        zoom_spring_friction: resolveScreenSpring(visualSettings).friction,
        zoom_spring_mass: resolveScreenSpring(visualSettings).mass,
        webcam_corner: visualSettings.webcamCorner ?? 'bottom-right',
        webcam_x: visualSettings.webcamX ?? 0.895,
        webcam_y: visualSettings.webcamY ?? 0.895,
        webcam_enabled: visualSettings.webcamEnabled !== false,
        webcam_layout: visualSettings.webcamLayout ?? 'overlay',
        webcam_keyframes: storeState.webcamKeyframes,
        webcam_silence_ranges: visualSettings.webcamEnabled === false ? [] : await loadCameraSilence(storeState),
        webcam_size: visualSettings.webcamSize ?? 0.15,
        webcam_shape: visualSettings.webcamShape ?? 'circle',
        webcam_mirrored: visualSettings.webcamMirrored ?? false,
      };

      const shortcuts = await shortcutExportPayload({...storeState,
        trimStart:exportConfig.trim_start,trimEnd:exportConfig.trim_end},
        dimensions.width,dimensions.height,exportConfig.frame_rate);
      const masks = maskExportPayload({...storeState,trimStart:exportConfig.trim_start,trimEnd:exportConfig.trim_end},exportConfig.frame_rate);
      const captions = await captionExportPayload({...storeState,trimStart:exportConfig.trim_start,trimEnd:exportConfig.trim_end},
        dimensions.width,dimensions.height,exportConfig.frame_rate);
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<{ current: number; total: number; percentage: number }>('export:progress', (event) => {
        setCanCancelExport(true);
        setExportProgress(Math.round(event.payload.percentage));
      });

      const outputPath = await invoke<string>('export_video', {
        inputPath: videoFilePath,
        settings: {...exportConfig,shortcut_overlay:shortcuts,caption_overlay:captions,mask_overlay:masks}
      });

      setExportProgress(100);
      setExportStatus({ message: `Export complete. Saved to: ${outputPath}` });
    } catch (error: unknown) {
      console.error('[Export] Export failed:', error);
      const message = error instanceof Error ? error.message : String(error);
      if (message === 'Export cancelled') setExportStatus({ message: 'Export cancelled.' });
      else setExportStatus({ message: `Export failed: ${message}`, error: true });
    } finally {
      unlisten?.();
      exportInFlight.current = false;
      setIsExporting(false);
      setIsCancellingExport(false);
      setCanCancelExport(false);
      setExportProgress(0);
    }
  };

  const handleCancelExport = async () => {
    if (!canCancelExport || isCancellingExport) return;
    setIsCancellingExport(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const accepted = await invoke<boolean>('cancel_export');
      if (!accepted) { setIsCancellingExport(false); setCanCancelExport(false); }
    } catch (error) {
      setIsCancellingExport(false);
      setExportStatus({ message: `Could not cancel export: ${String(error)}`, error: true });
    }
  };

  useEffect(() => {
    if (!isExporting) return;
    // Inert blocks pointer/focus interaction; document shortcuts need a guard too.
    const blockEditingKeys = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    document.addEventListener('keydown', blockEditingKeys, true);
    return () => document.removeEventListener('keydown', blockEditingKeys, true);
  }, [isExporting]);

  const handleToggleTimelineCollapse = useCallback(() => {
    setTimelineCollapsed(collapsed => !collapsed);
  }, []);

  const addFlag = useCallback(() => {
    const time = useEditorStore.getState().currentTime;
    setFlags((current) => [...current, time]);
  }, []);

  const runShortcutAction = (action: EditorShortcutAction) => {
    if (action === 'export') handleExport();
    if (action === 'play-pause') handlePlayPause();
    if (action === 'shortcuts') setShortcutsOpen(true);
    if (action === 'notes') setNotesOpen((open) => !open);
    if (action === 'flag') addFlag();
    if (action === 'close') onClose();
    setCommandOpen(false);
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      const key = event.key.toLowerCase();

      if (event.key === 'Escape') {
        if (commandOpen) setCommandOpen(false);
        else if (shortcutsOpen) setShortcutsOpen(false);
        else if (notesOpen) setNotesOpen(false);
        return;
      }

      if (event.metaKey && !event.ctrlKey && !event.altKey && key === '/') {
        event.preventDefault();
        setShortcutsOpen((open) => !open);
      } else if (event.metaKey && !event.ctrlKey && !event.altKey && key === 'k') {
        event.preventDefault();
        setCommandOpen((open) => !open);
      } else if (event.metaKey && event.altKey && !event.ctrlKey && key === '/') {
        event.preventDefault();
        setNotesOpen((open) => !open);
      } else if (event.metaKey && event.altKey && event.ctrlKey && key === 'f') {
        event.preventDefault();
        addFlag();
      } else if (event.metaKey && !event.altKey && !event.ctrlKey && key === 'e') {
        event.preventDefault();
        handleExport();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [commandOpen, shortcutsOpen, notesOpen, videoFilePath, addFlag]);

  // The timeline clock must advance through source cuts and empty gaps. Media
  // follows this monotonic clock; source time must never overwrite edited time.
  useEffect(() => {
    if (!isPlaying) return;
    let frame = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const state = useEditorStore.getState();
      const end = timelineDuration(state);
      const next = Math.min(end, state.currentTime + Math.max(0, now - last));
      last = now;
      state.setCurrentTime(next, 'playback');
      if (next >= end) { state.setIsPlaying(false); return; }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [isPlaying]);

  // Format the recorded date for the status bar
  const formattedRecordedDate = recordedAt
    ? new Date(recordedAt).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      })
    : 'Unknown';

  return (
    <div className="editor-view" aria-busy={isExporting}>
      <BackgroundAudioPreview />
      <BackgroundAudioDrop disabled={isExporting} onImported={()=>setAudioImportRevision(value=>value+1)} />
      {/* Top Bar */}
      <EditorTopBar
        projectName={projectTitle}
        onProjectNameChange={setProjectTitle}
        onExport={handleExport}
        onClose={onClose}
        isExporting={isExporting}
        exportProgress={exportProgress}
        onCancelExport={handleCancelExport}
        canCancelExport={canCancelExport}
        isCancellingExport={isCancellingExport}
      />

      {exportStatus && <div className="editor-export-status" data-error={exportStatus.error || undefined}>
        <span role={exportStatus.error ? 'alert' : 'status'}>{exportStatus.message}</span>
        <button type="button" onClick={() => setExportStatus(null)} aria-label="Dismiss export status">Dismiss</button>
      </div>}
      {/* Main Content Area */}
      <div className="editor-main-content" inert={isExporting}>
        {/* Video Preview Panel - Left */}
        <div className="editor-video-section">
          <VideoPreviewPanel
            isPlaying={isPlaying}
            onPlayPause={handlePlayPause}
            onSeek={handleSeek}
            showMouseOverlay={showMouseOverlay}
          />
        </div>

        {/* Properties Panel - Right */}
        <div className="editor-properties-section">
          <PropertiesPanel
            audioImportRevision={audioImportRevision}
            onShowMouseOverlay={setShowMouseOverlay}
            showMouseOverlay={showMouseOverlay}
            isExporting={isExporting}
          />
        </div>
      </div>

      {/* Timeline - Bottom */}
      <div className="editor-timeline-section" inert={isExporting}>
        <ProfessionalTimeline
          isCollapsed={timelineCollapsed}
          onToggleCollapse={handleToggleTimelineCollapse}
          isExporting={isExporting}
        />
      </div>

      {/* Status Bar */}
      <div className="editor-status-bar">
        <div className="status-bar-left">
          {/* Can add additional status info here later */}
        </div>
        <div className="status-bar-right">
          <span className="status-recorded-date">Recorded {formattedRecordedDate}</span>
        </div>
      </div>

      <EditorShortcutOverlays
        commandOpen={commandOpen}
        shortcutsOpen={shortcutsOpen}
        notesOpen={notesOpen}
        flags={flags}
        onCloseCommand={() => setCommandOpen(false)}
        onCloseShortcuts={() => setShortcutsOpen(false)}
        onCloseNotes={() => setNotesOpen(false)}
        onAction={runShortcutAction}
      />

      <style>{`
        .editor-status-bar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          height: 28px;
          padding: 0 16px;
          background: var(--editor-bg-secondary);
          border-top: 1px solid var(--editor-border);
          flex-shrink: 0;
        }

        .status-bar-left,
        .status-bar-right {
          display: flex;
          align-items: center;
          gap: 12px;
        }

        .status-recorded-date {
          font-size: 12px;
          color: var(--editor-text-secondary);
          font-weight: 400;
        }
      `}</style>
    </div>
  );
};

export default EditorView;
