import { edit, restoreEdit } from '../history';
import type { EditorState, EditorActions } from '../types';

type SetFn = (update: Partial<EditorState & EditorActions> | ((state: EditorState & EditorActions) => void)) => void;
type GetFn = () => EditorState & EditorActions;

export const createPlaybackActions = (set: SetFn, get: GetFn) => ({
  setIsPlaying: (isPlaying: boolean) => {
    const state = get();
    set({ isPlaying, playbackRevision: state.playbackRevision + Number(state.isPlaying !== isPlaying) });
  },

  setCurrentTime: (time: number, origin: 'seek' | 'playback' = 'seek') => {
    set({ currentTime: time, playbackRevision: get().playbackRevision + Number(origin === 'seek') });
  },

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
