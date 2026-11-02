import { invoke } from '@tauri-apps/api/core';
import { createRecordingLoadGuard } from '../recordingLoadGuard';
import type { EditorState, EditorActions } from '../types';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
export function createThumbnailActions(set: SetFn, get: () => EditorState & EditorActions) {
  const begin = createRecordingLoadGuard(get);
  return {
    loadThumbnails: async (filePath: string) => {
      if (filePath !== get().videoFilePath) return;
      const current = begin('thumbnails');
      const count = Math.max(5,Math.min(50,Math.ceil(get().duration/1000)));
      set(state=>{state.thumbnailsLoading=true;});
      try {
        const thumbnails = await invoke<string[]>('extract_video_thumbnails',{
          videoPath:filePath,thumbnailCount:count,thumbnailWidth:160,
        });
        if (!current()) return;
        set(state=>{state.thumbnails=thumbnails;state.thumbnailsLoading=false;});
      } catch(error) {
        if (!current()) return;
        console.error('Failed to load thumbnails:',error);
        set(state=>{state.thumbnailsLoading=false;});
      }
    },
  };
}
