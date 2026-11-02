import { exportTimeline } from '../../../../utils/timeline/exportTimeline';
import type {SilenceRange} from '../../../../utils/camera/silence';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { selectMotionInputs } from './motionInputs';
import { createLatestTaskQueue } from '../../../../utils/motion/latestTask';
import { invoke } from '@tauri-apps/api/core';
import { useEditorStore } from '../../../../stores/editor';
import { resolveCursorSpring } from '../../../../utils/motion/cursorStyle';
import { resolveScreenSpring } from '../../../../utils/motion/screenStyle';
import { exportZoomBlocks, type CameraFrame } from '../../../../utils/motion/zoomTrajectory';

const noCameraSettings = {};
const noSilence: readonly SilenceRange[] = [];

function useMotionRequest(includeCamera: boolean, silence:readonly SilenceRange[]) {
  const { videoFilePath, duration, zoomAnalysis, visualSettings, videoWidth, videoHeight, captureMode, screenResolution, recordingArea, webcamKeyframes, outputWidth, outputHeight, clips, tracks, trimStart, trimEnd } = useEditorStore(useShallow(selectMotionInputs));
  const cameraSettings = useMemo(() => includeCamera ? {
    webcam_enabled: visualSettings.webcamEnabled,
    webcam_silence_ranges: silence,
    webcam_size: visualSettings.webcamSize, webcam_shape: visualSettings.webcamShape,
    webcam_x: visualSettings.webcamX, webcam_y: visualSettings.webcamY,
    webcam_layout: visualSettings.webcamLayout, webcam_keyframes: webcamKeyframes,
  } : noCameraSettings, [includeCamera, silence, visualSettings.webcamEnabled, visualSettings.webcamSize,
    visualSettings.webcamShape, visualSettings.webcamX, visualSettings.webcamY,
    visualSettings.webcamLayout, webcamKeyframes]);
  const timing = useMemo(() => {
    try { return { value: duration > 0 ? exportTimeline({ clips, tracks, duration, trimStart, trimEnd }) : {}, error: null }; }
    catch (error) { return { value: {}, error: `Motion preview failed: ${String(error)}` }; }
  }, [clips, tracks, duration, trimStart, trimEnd]);
  const spring = resolveScreenSpring(visualSettings);
  const cursorSpring = resolveCursorSpring(visualSettings);
  const settings = useMemo(() => ({
    ...cameraSettings, ...timing.value,
    zoom_blocks: exportZoomBlocks(zoomAnalysis, visualSettings),
    animation_speed: visualSettings.zoomSpeedPreset,
    screen_motion_style: visualSettings.screenMotionStyle ?? null,
    zoom_spring_tension: spring.tension,
    zoom_spring_friction: spring.friction,
    zoom_spring_mass: spring.mass,
    source_crop: visualSettings.sourceCrop ?? null,
    source_width: videoWidth,
    source_height: videoHeight,
    capture_mode: captureMode,
    resolution: { width: outputWidth, height: outputHeight },
    visual_settings: { padding: visualSettings.padding, window_layout_mode: visualSettings.windowLayoutMode },
    screen_width: screenResolution?.width ?? null, screen_height: screenResolution?.height ?? null,
    window_width: recordingArea?.width ?? null, window_height: recordingArea?.height ?? null,
    cursor_settings: { loop_to_start: visualSettings.loopCursorPosition ?? false, loop_duration_ms: 500, spring_tension:cursorSpring.tension,spring_friction:cursorSpring.friction,spring_mass:cursorSpring.mass, shake_threshold: visualSettings.removeCursorShakes ? visualSettings.shakesThreshold : 0 },
  }), [cameraSettings, timing, visualSettings.loopCursorPosition, visualSettings.zoomEffectsEnabled, outputWidth, outputHeight, visualSettings.padding, visualSettings.windowLayoutMode, screenResolution, recordingArea, visualSettings.sourceCrop, visualSettings.removeCursorShakes, visualSettings.shakesThreshold, zoomAnalysis, visualSettings.zoomLeadInMs, visualSettings.zoomSpeedPreset, visualSettings.screenMotionStyle,
    cursorSpring.tension,cursorSpring.friction,cursorSpring.mass,
    spring.tension, spring.friction, spring.mass, videoWidth, videoHeight, captureMode]);
  return { videoFilePath, duration, settings, requestError: timing.error };
}

function useMotionTrajectory<T>(command: string, enabled: boolean, label: string, silence:readonly SilenceRange[]=noSilence) {
  const { videoFilePath, duration, settings, requestError } = useMotionRequest(command === 'compute_camera_opacity',silence);
  // A source path alone is insufficient: crop, timing and presentation edits
  // can replace the entire motion path without changing the recording file.
  const requestKey = useMemo(() => JSON.stringify({
    command, inputPath: videoFilePath, durationMs: Math.ceil(duration), settings,
  }), [command, videoFilePath, duration, settings]);
  const [result, setResult] = useState<{ key: string; frames: T[]; error: string | null }>();

  const queue = useRef<ReturnType<typeof createLatestTaskQueue> | null>(null);
  if (!queue.current) queue.current = createLatestTaskQueue();

  useEffect(() => {
    let cancelTask: (() => void) | undefined;
    if (!enabled || !videoFilePath || duration <= 0 || requestError) return;
    // Coalesce rapid edits before IPC. Once a calculation starts, let it finish
    // but retain only the newest pending request instead of flooding native workers.
    const timer = setTimeout(() => {
      cancelTask = queue.current!.submit(
        () => invoke<T[]>(command, { inputPath: videoFilePath, durationMs: Math.ceil(duration), settings }),
        frames => setResult({ key: requestKey, frames, error: null }),
        error => setResult({ key: requestKey, frames: [], error: `${label} preview failed: ${String(error)}` }),
      );
    }, 80);
    return () => { cancelTask?.(); clearTimeout(timer); };
  }, [videoFilePath, duration, settings, command, enabled, label, requestKey, requestError]);

  const current = enabled && result?.key === requestKey;
  return {
    frames: current ? result.frames : [] as T[],
    error: requestError ?? (current ? result.error : null),
    pending: !requestError && enabled && !!videoFilePath && duration > 0 && !current,
  };
}

export function useCameraTrajectory() {
  return useMotionTrajectory<CameraFrame>('compute_zoom_trajectory', true, 'Camera');
}

export function useLoupeTrajectory(enabled: boolean) {
  return useMotionTrajectory<import('../../../../utils/motion/loupe').LoupeFrame>(
    'compute_loupe_trajectory', enabled, 'Magnifier');
}

export interface CameraOpacityFrame { stage: number; lens_content: number; rect?: [number,number,number,number] | null }
export function useCameraOpacityTrajectory(enabled: boolean, silence:readonly SilenceRange[]=noSilence) {
  return useMotionTrajectory<CameraOpacityFrame>('compute_camera_opacity', enabled, 'Camera fade',silence);
}
