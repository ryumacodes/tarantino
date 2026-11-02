import { edit, restoreEdit } from '../history';
import type { EditorState, EditorActions } from '../types';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

export const createPlaybackActions = (set: SetFn, get: GetFn) => ({
  setIsPlaying: (isPlaying: boolean) => set((state) => {
    if(state.isPlaying!==isPlaying)state.playbackRevision=state.playbackRevision + 1;
    state.isPlaying = isPlaying;
  }),

  setCurrentTime: (time: number, origin: 'seek' | 'playback' = 'seek') => set((state) => {
    if(origin==='seek')state.playbackRevision=state.playbackRevision + 1;
    state.currentTime = time;
  }),

  setDuration: (duration: number) => set((state) => {
    state.duration = duration;
    if (state.trimEnd === state.duration || state.trimEnd === 0) {
      state.trimEnd = duration;
    }
  }),

  setProjectTitle: (title: string) => edit(set, (state) => {
    state.projectTitle = title;
  }),

  setTrimStart: (time: number) => edit(set, (state) => {
    state.trimStart = time;
  }),

  setTrimEnd: (time: number) => edit(set, (state) => {
    state.trimEnd = time;
  }),

  selectKeyframe: (keyframe: EditorState['selectedKeyframe']) => set((state) => {
    state.selectedKeyframe = keyframe;
  }),

  beginHistoryGroup: () => set(state => {
    if (state.historyGroup) state.historyGroup.depth++;
    else state.historyGroup = { depth: 1, recorded: false };
  }),
  endHistoryGroup: () => set(state => {
    if (state.historyGroup && --state.historyGroup.depth === 0) state.historyGroup = null;
  }),
  undo: () => set(state => restoreEdit(state, -1)),
  redo: () => set(state => restoreEdit(state, 1)),
});
