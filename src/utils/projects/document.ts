import {parseMasks} from "../masks/document";
import {parseCaptionSettings} from "../captions/settings";
import {parseTranscripts} from "../captions/document";
import { parseShortcutSettings } from "../shortcuts/settings";
import { resolveSourceCrop } from '../sourceCrop';
import type { EditorState } from '../../stores/editor/types';
import { DEFAULT_VISUAL_SETTINGS, DEFAULT_EXPORT_SETTINGS } from '../../stores/editor/constants';

const keys = [
  'videoFilePath', 'projectTitle', 'recordedAt', 'duration', 'trimStart', 'trimEnd',
  'zoomKeyframes', 'webcamKeyframes', 'overlays', 'audioSettings', 'hasWebcam',
  'hasMicrophone', 'hasSystemAudio', 'zoomAnalysis', 'videoWidth', 'videoHeight',
  'displayResolution', 'screenResolution', 'scaleFactor', 'recordingArea', 'captureMode',
  'masks', 'transcripts', 'captionSettings', 'shortcutSettings', 'dismissedTypingRanges', 'tracks', 'clips', 'visualSettings', 'exportSettings',
] as const satisfies readonly (keyof EditorState)[];
export type SavedEditorState = Pick<EditorState, typeof keys[number]>;
export interface ProjectDocument { format: 'tarantino-project'; version: 1; state: SavedEditorState }

export function createProjectDocument(state: EditorState): ProjectDocument {
  if (!state.videoFilePath) throw new Error('No recording is open');
  const saved = Object.fromEntries(keys.map(key => [key, state[key]]));
  return parseProjectDocument(JSON.parse(JSON.stringify({ format: 'tarantino-project', version: 1, state: saved })));
}
function object(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function finite(value: unknown): value is number { return typeof value === 'number' && Number.isFinite(value); }
function check(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(`Invalid project: ${message}`);
}
export function parseProjectDocument(value: unknown): ProjectDocument {
  check(object(value) && value.format === 'tarantino-project' && value.version === 1, 'unsupported format or version');
  const state = object(value.state) ? {...value.state} : value.state;
  check(object(state), 'missing editor state');
  state.captionSettings = parseCaptionSettings(state.captionSettings);
  state.transcripts = parseTranscripts(state.transcripts);
  state.masks = parseMasks(state.masks);
  state.shortcutSettings = parseShortcutSettings(state.shortcutSettings);
  if (!("dismissedTypingRanges" in state)) state.dismissedTypingRanges = [];
  check(Array.isArray(state.dismissedTypingRanges) && state.dismissedTypingRanges.every((n: unknown) => finite(n) && n >= 0), 'dismissed typing ranges');
  for (const key of keys) check(key in state, `missing ${key}`);
  check(typeof state.videoFilePath === 'string' && state.videoFilePath.length > 0, 'missing recording');
  check(typeof state.projectTitle === 'string', 'missing title');
  check(finite(state.duration) && state.duration >= 0, 'invalid duration');
  check(finite(state.trimStart) && finite(state.trimEnd) && state.trimStart >= 0
    && state.trimEnd >= state.trimStart, 'invalid trim range');
  for (const key of ['hasWebcam', 'hasMicrophone', 'hasSystemAudio']) check(typeof state[key] === 'boolean', key);
  for (const key of ['clips', 'tracks', 'zoomKeyframes', 'webcamKeyframes', 'overlays']) check(Array.isArray(state[key]), key);
  for (const clip of state.clips) {
    check(object(clip) && typeof clip.id === 'string' && typeof clip.trackId === 'string', 'clip identity');
    for (const key of ['micVolume', 'systemVolume']) check(clip[key] === undefined || (finite(clip[key]) && clip[key] >= 0 && clip[key] <= 1), `clip ${key}`);
    check(clip.disableCursorSmoothing === undefined || typeof clip.disableCursorSmoothing === 'boolean', 'clip cursor smoothing');
    check(clip.hideCursor === undefined || typeof clip.hideCursor === 'boolean', 'clip cursor visibility');
    check(['video', 'audio', 'webcam', 'screen'].includes(clip.type), 'clip type');
    for (const key of ['startTime', 'duration', 'sourceIn', 'sourceOut']) check(finite(clip[key]) && clip[key] >= 0, `clip ${key}`);
    check(clip.sourceOut >= clip.sourceIn && finite(clip.playbackRate) && clip.playbackRate > 0, 'clip source range or speed');
  }
  check(new Set(state.clips.map((clip: any) => clip.id)).size === state.clips.length, 'duplicate clip identifiers');
  for (const track of state.tracks) check(object(track) && typeof track.id === 'string' && Array.isArray(track.clips), 'track');
  check(state.clips.every((clip: any) => state.tracks.some((track: any) => track.id === clip.trackId)), 'clip references missing track');
  check(state.trimEnd <= state.clips.reduce((end:number,c:any)=>Math.max(end,c.startTime+c.duration),state.duration), 'trim exceeds edited timeline');
  for (const camera of state.webcamKeyframes) {
    check(object(camera) && typeof camera.id === 'string' && finite(camera.time) && camera.time >= 0, 'camera keyframe');
    for (const key of ['x','y','size']) check(finite(camera[key]) && camera[key]>=0 && camera[key]<=1, `camera ${key}`);
    check(camera.size>=0.01 && ['circle','roundrect'].includes(camera.shape) && typeof camera.visible==='boolean', 'camera appearance');
    check(camera.layout===undefined || ['overlay','fullscreen','hidden'].includes(camera.layout), 'camera layout');
  }
  check(object(state.visualSettings) && object(state.exportSettings) && object(state.audioSettings), 'settings');
  if (state.visualSettings.sourceCrop !== undefined) {
    check(object(state.visualSettings.sourceCrop), 'source crop');
    resolveSourceCrop(state.visualSettings.sourceCrop as any,state.videoWidth,state.videoHeight);
  }
  const background = state.audioSettings.backgroundAudio;
  if (background !== undefined) {
    check(object(background) && typeof background.path === 'string' && background.path.length > 0
      && typeof background.name === 'string' && finite(background.volume) && background.volume >= 0 && background.volume <= 1
      && typeof background.muted === 'boolean', 'background audio');
  }
  for (const key of ['micMuted', 'systemMuted']) check(state.audioSettings[key] === undefined || typeof state.audioSettings[key] === 'boolean', `audio ${key}`);
  for (const [key, template] of Object.entries(DEFAULT_VISUAL_SETTINGS)) {
    const entry = state.visualSettings[key];
    if (entry === undefined) continue;
    check(typeof entry === typeof template && (typeof entry !== 'number' || finite(entry)), `visual setting ${key}`);
  }
  check(state.visualSettings.webcamHideSilenceMs==null || (finite(state.visualSettings.webcamHideSilenceMs) && state.visualSettings.webcamHideSilenceMs>=100 && state.visualSettings.webcamHideSilenceMs<=60000), 'camera silence duration');
  check(state.visualSettings.webcamEnabled === undefined || typeof state.visualSettings.webcamEnabled === 'boolean', 'camera enabled');
  check(state.visualSettings.webcamLayout===undefined || ['overlay','fullscreen','hidden'].includes(state.visualSettings.webcamLayout), 'camera layout');
  check(state.visualSettings.zoomEffectsEnabled === undefined || typeof state.visualSettings.zoomEffectsEnabled === 'boolean', 'zoom effects');
  check(['slow', 'mellow', 'quick', 'rapid'].includes(state.visualSettings.zoomSpeedPreset), 'zoom speed');
  check(['slow', 'mellow', 'quick', 'rapid'].includes(state.visualSettings.cursorSpeedPreset), 'cursor speed');
  check(state.visualSettings.zoomLeadInMs === undefined || (Number.isFinite(state.visualSettings.zoomLeadInMs) && state.visualSettings.zoomLeadInMs >= 0 && state.visualSettings.zoomLeadInMs <= 2000), 'zoom lead-in');
  check(state.visualSettings.screenMotionStyle === undefined || ['focused', 'smooth'].includes(state.visualSettings.screenMotionStyle), 'screen style');
  check(state.visualSettings.cursorRotationMode === undefined || ['horizontal', 'directional'].includes(state.visualSettings.cursorRotationMode), 'cursor rotation mode');
  check(state.visualSettings.cursorMotionStyle === undefined || ['smooth', 'medium', 'rapid', 'none'].includes(state.visualSettings.cursorMotionStyle), 'cursor motion style');
  if (state.zoomAnalysis !== null) {
    check(object(state.zoomAnalysis) && Array.isArray(state.zoomAnalysis.zoom_blocks), 'zoom analysis');
    for (const block of state.zoomAnalysis.zoom_blocks) {
      check(object(block) && typeof block.id === 'string', 'zoom block');
      check(block.is_disabled === undefined || typeof block.is_disabled === 'boolean', 'zoom enabled state');
      // Native Option values serialize to null when no magnifier is enabled.
      if (block.loupe != null) {
        const lens = block.loupe;
        check(object(lens) && typeof lens.glass === 'boolean', 'magnifier style');
        for (const key of ['radius', 'bevel', 'chromatic_aberration', 'shadow_alpha'])
          check(finite(lens[key]) && lens[key] >= 0, `magnifier ${key}`);
        check(lens.radius > 0 && lens.shadow_alpha <= 1, 'magnifier bounds');
      }
      for (const key of ['start_time', 'end_time', 'center_x', 'center_y', 'zoom_factor']) check(finite(block[key]), `zoom ${key}`);
      check(block.end_time >= block.start_time && block.zoom_factor >= 1, 'zoom range');
    }
  }
  const saved = Object.fromEntries(keys.map(key => [key, structuredClone(state[key])])) as SavedEditorState;
  saved.visualSettings = { ...DEFAULT_VISUAL_SETTINGS, ...saved.visualSettings,
    screenMotionStyle: saved.visualSettings.screenMotionStyle,
    cursorMotionStyle: saved.visualSettings.cursorMotionStyle };
  saved.exportSettings = { ...DEFAULT_EXPORT_SETTINGS, ...saved.exportSettings };
  // Track clip lists are derived from the canonical edit list, never separately restored copies.
  saved.tracks = saved.tracks.map(track => ({ ...track, clips: saved.clips.filter(clip => clip.trackId === track.id) }));
  return { format: 'tarantino-project', version: 1, state: saved };
}
