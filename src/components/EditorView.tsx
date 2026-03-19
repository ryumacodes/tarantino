import React, { useState, useEffect } from 'react';
import EditorTopBar from './editor/EditorTopBar';
import VideoPreviewPanel from './editor/VideoPreviewPanel';
import PropertiesPanel from './editor/PropertiesPanel';
import ProfessionalTimeline from './editor/ProfessionalTimeline';
import { useEditorStore, SPRING_PRESETS } from '../stores/editor';
import '../styles/dracula-theme.css';

interface EditorViewProps {
  onClose: () => void;
}

const EditorView: React.FC<EditorViewProps> = ({ onClose }) => {
  const [showMouseOverlay, setShowMouseOverlay] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);
  const [timelineCollapsed, setTimelineCollapsed] = useState(false);
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

      // Get projectTitle for export filename
      const projectTitleForExport = storeState.projectTitle || 'Untitled Recording';

      // Build export configuration from store settings
      const exportConfig = {
        output_path: null, // Let backend choose path based on format
        project_title: projectTitleForExport, // Used for output filename
        resolution: dimensions,
        frame_rate: exportSettings.frameRate,
        quality: exportSettings.quality,
        format: exportSettings.format,
        codec: exportSettings.format === 'mov' ? 'prores' : 'h264',
        trim_start: trimStart,
        trim_end: trimEnd,
        // Zoom blocks for effects rendering
        zoom_blocks: zoomBlocks.map((block: any) => ({
          start_time_ms: block.start_time,
          end_time_ms: block.end_time,
          zoom_level: block.zoom_factor,
          center_x: block.center_x,
          center_y: block.center_y,
          kind: block.kind ?? 'click',
          zoom_in_speed: block.zoom_in_speed ?? null,
          zoom_out_speed: block.zoom_out_speed ?? null,
        })),
        // Legacy zoom data for compatibility
        zoom_keyframes: zoomKeyframes,
        zoom_analysis: zoomAnalysis,
        // Visual settings for background/padding/shadows
        visual_settings: {
          background_type: visualSettings.backgroundType,
          background_color: visualSettings.backgroundColor,
          gradient_direction: visualSettings.gradientDirection,
          gradient_stops: visualSettings.gradientStops,
          padding: visualSettings.padding,
          corner_radius: visualSettings.cornerRadius,
          shadow_enabled: visualSettings.shadowEnabled,
          shadow_intensity: visualSettings.shadowIntensity,
          shadow_blur: visualSettings.shadowBlur,
          shadow_offset_x: visualSettings.shadowOffsetX,
          shadow_offset_y: visualSettings.shadowOffsetY,
          aspect_ratio: visualSettings.aspectRatio,
          // Motion blur settings for Screen Studio-style effects
          motion_blur_enabled: visualSettings.motionBlurEnabled,
          motion_blur_pan_intensity: visualSettings.motionBlurPanIntensity / 100,
          motion_blur_zoom_intensity: visualSettings.motionBlurZoomIntensity / 100,
          motion_blur_cursor_intensity: visualSettings.motionBlurCursorIntensity / 100,
          // Device frame settings
          device_frame: visualSettings.deviceFrame,
          device_frame_color: visualSettings.deviceFrameColor,
        },
        // Cursor settings - pass all visual settings for cursor rendering in export
        cursor_settings: {
          enabled: !visualSettings.hideCursor,
          size: visualSettings.cursorScale ?? 1.0,
          highlight_clicks: true,
          smoothing: visualSettings.cursorSmoothing ?? 0.15,
          // Visual style
          style: visualSettings.cursorStyle ?? 'pointer',
          always_use_pointer: visualSettings.alwaysUsePointer ?? false,
          color: visualSettings.cursorColor ?? '#ffffff',
          highlight_color: visualSettings.cursorHighlightColor ?? '#ff6b6b',
          ripple_color: visualSettings.cursorRippleColor ?? '#64b4ff',
          shadow_intensity: visualSettings.cursorShadowIntensity ?? 30,
          // Trail settings
          trail_enabled: visualSettings.cursorTrailEnabled ?? false,
          trail_length: visualSettings.cursorTrailLength ?? 10,
          trail_opacity: visualSettings.cursorTrailOpacity ?? 0.5,
          // Click effect
          click_effect: visualSettings.clickEffect ?? 'ripple',
          // Spring physics — resolve preset to actual values (single source of truth)
          speed_preset: visualSettings.cursorSpeedPreset ?? 'mellow',
          spring_tension: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].tension,
          spring_friction: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].friction,
          spring_mass: SPRING_PRESETS[visualSettings.cursorSpeedPreset ?? 'mellow'].mass,
          // Rotation
          rotation: visualSettings.cursorRotation ?? 0,
          rotate_while_moving: visualSettings.rotateCursorWhileMoving ?? false,
          rotation_intensity: visualSettings.rotationIntensity ?? 50,
          // Idle behavior
          hide_when_idle: visualSettings.hideCursorWhenIdle ?? true,
          idle_timeout: visualSettings.idleTimeout ?? 3000,
        },
        // Audio settings
        audio_settings: {
          mic_gain: audioSettings.micGain,
          system_gain: audioSettings.systemGain,
          noise_gate: audioSettings.noiseGate,
          dual_track: audioSettings.dualTrack,
        },
        // Animation settings — resolve preset to actual spring values (single source of truth)
        animation_speed: visualSettings.zoomSpeedPreset ?? 'mellow',
        zoom_spring_tension: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].tension,
        zoom_spring_friction: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].friction,
        zoom_spring_mass: SPRING_PRESETS[visualSettings.zoomSpeedPreset ?? 'mellow'].mass,
      };

      console.log('[Export] Starting export with full pipeline settings:', exportConfig);

      // Listen for real-time progress events from backend
      console.log('[Export] Setting up progress event listener...');
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<{ current: number; total: number; percentage: number }>('export:progress', (event) => {
        console.log('[Export] Progress event received:', event.payload);
        setExportProgress(Math.round(event.payload.percentage));
      });
      console.log('[Export] Progress listener set up');

      // Call the export command
      console.log('[Export] Calling invoke("export_video") with inputPath:', videoFilePath);
      const outputPath = await invoke<string>('export_video', {
        inputPath: videoFilePath,
        settings: exportConfig
      });
      console.log('[Export] invoke returned outputPath:', outputPath);

      setExportProgress(100);
      unlisten(); // Clean up listener

      console.log('[Export] Export completed successfully:', outputPath);

      // Show success message
      setTimeout(() => {
        setIsExporting(false);
        setExportProgress(0);
        alert(`Export completed successfully!\nSaved to: ${outputPath}`);
      }, 1000);

    } catch (error: any) {
      console.error('[Export] Export failed with error:', error);
      console.error('[Export] Error type:', typeof error);
      console.error('[Export] Error message:', error?.message);
      console.error('[Export] Error stack:', error?.stack);
      console.error('[Export] Full error object:', JSON.stringify(error, null, 2));
      setIsExporting(false);
      setExportProgress(0);
      alert(`Export failed: ${error}`);
    }
  };

  const handleShowMouseOverlay = (show: boolean) => {
    setShowMouseOverlay(show);
  };

  const handleToggleTimelineCollapse = () => {
    setTimelineCollapsed(!timelineCollapsed);
  };

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