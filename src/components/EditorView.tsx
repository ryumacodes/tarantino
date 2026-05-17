import React, { useState, useEffect } from 'react';
import EditorTopBar from './editor/EditorTopBar';
import VideoPreviewPanel from './editor/VideoPreviewPanel';
import PropertiesPanel from './editor/PropertiesPanel';
import ProfessionalTimeline from './editor/ProfessionalTimeline';
import { EditorShortcutOverlays, type EditorShortcutAction } from './editor/EditorShortcutOverlays';
import { useEditorStore, SPRING_PRESETS, type ZoomBlock } from '../stores/editor';
import '../styles/dracula-theme.css';

interface EditorViewProps {
  onClose: () => void;
}

const EditorView: React.FC<EditorViewProps> = ({ onClose }) => {
  const [showMouseOverlay, setShowMouseOverlay] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);
  const [timelineCollapsed, setTimelineCollapsed] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [notesOpen, setNotesOpen] = useState(false);
  const [flags, setFlags] = useState<number[]>([]);
  const {
    duration,
    currentTime,
    setCurrentTime,
    isPlaying,
    setIsPlaying,
    videoFilePath,
    trimStart,
    trimEnd,
    zoomKeyframes,
    zoomAnalysis,
    audioSettings,
    projectTitle,
    recordedAt,
    setProjectTitle
  } = useEditorStore();

  const handlePlayPause = () => {
    if (!isPlaying && currentTime >= duration) {
      setCurrentTime(0);
    }
    setIsPlaying(!isPlaying);
  };

  const handleSeek = (time: number) => {
    setCurrentTime(time);
  };

  const handleExport = async () => {
    if (!videoFilePath) return;

    setIsExporting(true);
    setExportProgress(0);

    try {
      const { invoke } = await import('@tauri-apps/api/core');

      const storeState = useEditorStore.getState();
      const exportSettings = storeState.exportSettings;
      const visualSettings = storeState.visualSettings;
      const zoomBlocks = storeState.zoomAnalysis?.zoom_blocks || [];
      const dimensions = storeState.getExportDimensions();

      const projectTitleForExport = storeState.projectTitle || 'Untitled Recording';

      const exportConfig = {
        output_path: null,
        project_title: projectTitleForExport,
        resolution: dimensions,
        frame_rate: exportSettings.frameRate,
        quality: exportSettings.quality,
        format: exportSettings.format,
        codec: exportSettings.format === 'mov' ? 'prores' : 'h264',
        trim_start: trimStart,
        trim_end: trimEnd,
        zoom_blocks: zoomBlocks.map((block: ZoomBlock) => ({
          start_time_ms: block.start_time,
          end_time_ms: block.end_time,
          zoom_level: block.zoom_factor,
          center_x: block.center_x,
          center_y: block.center_y,
          kind: block.kind ?? 'click',
          zoom_in_speed: block.zoom_in_speed ?? null,
          zoom_out_speed: block.zoom_out_speed ?? null,
          centers: block.centers ?? [],
        })),
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
          spring_tension: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].tension,
          spring_friction: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].friction,
          spring_mass: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].mass,
          rotation: visualSettings.cursorRotation ?? 0,
          rotate_while_moving: visualSettings.rotateCursorWhileMoving ?? false,
          rotation_intensity: visualSettings.rotationIntensity ?? 50,
          hide_when_idle: visualSettings.hideCursorWhenIdle ?? true,
          idle_timeout: visualSettings.idleTimeout ?? 3000,
          stop_at_end: visualSettings.stopCursorAtEnd ?? false,
          stop_duration_ms: visualSettings.stopCursorDuration ?? 0,
          loop_to_start: visualSettings.loopCursorPosition ?? false,
          loop_duration_ms: 500,
        },
        audio_settings: {
          mic_gain: audioSettings.micGain,
          system_gain: audioSettings.systemGain,
          noise_gate: audioSettings.noiseGate,
          dual_track: audioSettings.dualTrack,
        },
        source_width: storeState.videoWidth ?? null,
        source_height: storeState.videoHeight ?? null,
        capture_mode: storeState.captureMode ?? 'display',
        screen_width: storeState.screenResolution?.width ?? null,
        screen_height: storeState.screenResolution?.height ?? null,
        animation_speed: visualSettings.zoomSpeedPreset ?? 'mellow',
        zoom_spring_tension: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].tension,
        zoom_spring_friction: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].friction,
        zoom_spring_mass: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].mass,
        webcam_corner: visualSettings.webcamCorner ?? 'bottom-right',
        webcam_x: visualSettings.webcamX ?? 0.895,
        webcam_y: visualSettings.webcamY ?? 0.895,
        webcam_size: visualSettings.webcamSize ?? 0.15,
        webcam_shape: visualSettings.webcamShape ?? 'circle',
      };

      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<{ current: number; total: number; percentage: number }>('export:progress', (event) => {
        setExportProgress(Math.round(event.payload.percentage));
      });

      const outputPath = await invoke<string>('export_video', {
        inputPath: videoFilePath,
        settings: exportConfig
      });

      setExportProgress(100);
      unlisten();

      setTimeout(() => {
        setIsExporting(false);
        setExportProgress(0);
        alert(`Export completed successfully!\nSaved to: ${outputPath}`);
      }, 1000);
    } catch (error: unknown) {
      console.error('[Export] Export failed:', error);
      setIsExporting(false);
      setExportProgress(0);
      const message = error instanceof Error ? error.message : String(error);
      alert(`Export failed: ${message}`);
    }
  };

  const handleShowMouseOverlay = (show: boolean) => {
    setShowMouseOverlay(show);
  };

  const handleToggleTimelineCollapse = () => {
    setTimelineCollapsed(!timelineCollapsed);
  };

  const addFlag = () => {
    setFlags((current) => [...current, currentTime]);
  };

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
  }, [commandOpen, shortcutsOpen, notesOpen, currentTime]);

  // Auto-play timer
  useEffect(() => {
    if (isPlaying) {
      const interval = setInterval(() => {
        const nextTime = currentTime + 16.67; // 60fps
        if (nextTime >= duration) {
          setIsPlaying(false);
          setCurrentTime(duration);
        } else {
          setCurrentTime(nextTime);
        }
      }, 16.67);
      return () => clearInterval(interval);
    }
  }, [isPlaying, currentTime, duration, setCurrentTime]);

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
    <div className="editor-view">
      {/* Top Bar */}
      <EditorTopBar
        projectName={projectTitle}
        onProjectNameChange={setProjectTitle}
        onExport={handleExport}
        onClose={onClose}
        isExporting={isExporting}
        exportProgress={exportProgress}
      />

      {/* Main Content Area */}
      <div className="editor-main-content">
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
            onShowMouseOverlay={handleShowMouseOverlay}
            showMouseOverlay={showMouseOverlay}
            isExporting={isExporting}
          />
        </div>
      </div>

      {/* Timeline - Bottom */}
      <div className="editor-timeline-section">
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
