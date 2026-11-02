import {createMaskActions} from './maskActions';
import {parseMasks,type RecordingMask} from "../../../utils/masks/document";
import {parseCaptionSettings,type CaptionSettings} from "../../../utils/captions/settings";
import {parseTranscripts,type TranscriptSession} from "../../../utils/captions/document";
import { parseShortcutSettings, type ShortcutSettings } from "../../../utils/shortcuts/settings";
import { parseStylePreset, type StylePreset } from '../../../utils/presets/stylePreset';
import { edit } from '../history';
import { resolveSourceCrop } from '../../../utils/sourceCrop';

import type {
  EditorState,
  EditorActions,
  Overlay,
  AudioSettings,
  VisualSettings,
  ExportSettings,
} from '../types';

import {
  DEFAULT_VISUAL_SETTINGS,
  DEFAULT_EXPORT_SETTINGS,
  RESOLUTION_DIMENSIONS,
  ASPECT_RATIOS,
  WALLPAPERS,
} from '../constants';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

export const createSettingsActions = (set: SetFn, get: GetFn) => ({
  ...createMaskActions(set,get),
  applyStylePreset: (input: StylePreset) => {
    const preset=parseStylePreset(input);
    edit(set,state=>{
      const sourceCrop=state.visualSettings.sourceCrop;
      const backgroundAudio=state.audioSettings.backgroundAudio;
      state.visualSettings={...preset.visual,sourceCrop};
      state.audioSettings={...preset.audio,backgroundAudio};
      state.exportSettings=preset.output;
      state.isPlaying=false;
      state.playbackRevision+=1;
    });
  },
  updateCaptionSettings: (settings:Partial<CaptionSettings>) => edit(set,state=>{
    state.captionSettings=parseCaptionSettings({...state.captionSettings,...settings});
  }),
  setMasks: (input:RecordingMask[]) => {
    const masks=parseMasks(input);
    edit(set,state=>{state.masks=masks;});
  },
  setTranscriptSession: (session:TranscriptSession) => {
    const current=get().transcripts;
    const transcripts=parseTranscripts(current.some(item=>item.id===session.id)
      ?current.map(item=>item.id===session.id?session:item):[...current,session]);
    edit(set,state=>{state.transcripts=transcripts;});
  },
  setTranscriptWordHidden: (sessionId:string,wordId:string,hidden:boolean) => {
    if(typeof hidden!=='boolean')throw new Error('Invalid transcript visibility');
    edit(set,state=>{
      const word=state.transcripts.find(item=>item.id===sessionId)?.words.find(item=>item.id===wordId);
      if(word)word.isHidden=hidden;
    });
  },
  updateTranscriptText: (sessionId:string,wordId:string,text:string) => {
    if(typeof text!=='string'||text.length>10000)throw new Error('Invalid transcript text');
    edit(set,state=>{
      const word=state.transcripts.find(item=>item.id===sessionId)?.words.find(item=>item.id===wordId);
      if(word)word.text=text;
    });
  },
  updateShortcutSettings: (settings: Partial<ShortcutSettings>) => edit(set, state => {
    state.shortcutSettings = parseShortcutSettings({...state.shortcutSettings,...settings});
  }),

  addOverlay: (overlay: Overlay) => edit(set, (state) => {
    state.overlays.push(overlay);
  }),

  updateOverlay: (id: string, updates: Partial<Overlay>) => edit(set, (state) => {
    const index = state.overlays.findIndex(o => o.id === id);
    if (index !== -1) {
      Object.assign(state.overlays[index], updates);
    }
  }),

  deleteOverlay: (id: string) => edit(set, (state) => {
    state.overlays = state.overlays.filter(o => o.id !== id);
  }),

  updateAudioSettings: (settings: Partial<AudioSettings>) => edit(set, (state) => {
    Object.assign(state.audioSettings, settings);
  }),

  updateVisualSettings: (settings: Partial<VisualSettings>) => edit(set, (state) => {
    Object.assign(state.visualSettings, settings);
  }),

  resetVisualSettings: () => edit(set, (state) => {
    state.visualSettings = { ...DEFAULT_VISUAL_SETTINGS };
  }),

  applyWallpaper: (wallpaperId: string) => edit(set, (state) => {
    const wallpaper = WALLPAPERS[wallpaperId as keyof typeof WALLPAPERS];
    if (!wallpaper) {
      console.warn('Unknown wallpaper:', wallpaperId);
      return;
    }

    state.visualSettings.wallpaperId = wallpaperId;
    state.visualSettings.customBackgroundImage = null;
    state.visualSettings.backgroundType = 'wallpaper';

    if (wallpaper.type === 'gradient') {
      const colors = [...(wallpaper as { type: 'gradient'; colors: readonly string[] }).colors];
      state.visualSettings.gradientStops = colors.map((color, i) => ({
        color,
        position: (i / (colors.length - 1)) * 100
      }));
    } else if (wallpaper.type === 'solid') {
      state.visualSettings.backgroundColor = (wallpaper as { type: 'solid'; color: string }).color;
    }
  }),

  applyCustomWallpaper: (imageDataUrl: string) => edit(set, (state) => {
    state.visualSettings.backgroundType = 'wallpaper';
    state.visualSettings.wallpaperId = null;
    state.visualSettings.customBackgroundImage = imageDataUrl;
  }),

  updateExportSettings: (settings: Partial<ExportSettings>) => edit(set, (state) => {
    Object.assign(state.exportSettings, settings);
  }),

  resetExportSettings: () => edit(set, (state) => {
    state.exportSettings = { ...DEFAULT_EXPORT_SETTINGS };
  }),

  getExportDimensions: () => {
    const state = get();
    const { resolution, customWidth, customHeight } = state.exportSettings;

    if (resolution === 'custom' && customWidth && customHeight) {
      return { width: customWidth, height: customHeight };
    }

    const presetHeight = (RESOLUTION_DIMENSIONS[resolution as keyof typeof RESOLUTION_DIMENSIONS] || RESOLUTION_DIMENSIONS['1080p']).height;
    const aspectRatio = state.visualSettings.aspectRatio || 'auto';

    if (aspectRatio === 'auto') {
      // Use source video aspect ratio at preset height
      const crop = state.visualSettings.sourceCrop && state.videoWidth && state.videoHeight
        ? resolveSourceCrop(state.visualSettings.sourceCrop,state.videoWidth,state.videoHeight) : undefined;
      const srcW = crop?.width ?? state.videoWidth;
      const srcH = crop?.height ?? state.videoHeight;
      if (srcW && srcH && srcH > 0) {
        const w = Math.round((presetHeight * srcW) / srcH);
        // Ensure even dimensions for FFmpeg
        return { width: w % 2 === 0 ? w : w + 1, height: presetHeight };
      }
      // Fallback: 16:9 if no source dims
      return RESOLUTION_DIMENSIONS[resolution as keyof typeof RESOLUTION_DIMENSIONS] || RESOLUTION_DIMENSIONS['1080p'];
    }

    // Explicit aspect ratio: compute width from ratio at preset height
    const ratioInfo = ASPECT_RATIOS[aspectRatio as keyof typeof ASPECT_RATIOS];
    if (ratioInfo && ratioInfo.width > 0 && ratioInfo.height > 0) {
      const w = Math.round((presetHeight * ratioInfo.width) / ratioInfo.height);
      return { width: w % 2 === 0 ? w : w + 1, height: presetHeight };
    }

    return RESOLUTION_DIMENSIONS[resolution as keyof typeof RESOLUTION_DIMENSIONS] || RESOLUTION_DIMENSIONS['1080p'];
  }
});
