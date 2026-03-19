// Playback Actions
import type { EditorState, EditorActions } from '../types';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

export const createPlaybackActions = (set: SetFn, get: GetFn) => ({
  setIsPlaying: (isPlaying: boolean) => set((state) => {
    console.log(`Editor Store: setIsPlaying -> ${isPlaying}`);
    state.isPlaying = isPlaying;
  }),

  setCurrentTime: (time: number) => set((state) => {
    state.currentTime = time;
  }),

  setDuration: (duration: number) => set((state) => {
    console.log(`Editor Store: setDuration -> ${duration}`);
    state.duration = duration;
    if (state.trimEnd === state.duration || state.trimEnd === 0) {
      state.trimEnd = duration;
    }
  }),

  setProjectTitle: (title: string) => set((state) => {
    console.log(`Editor Store: setProjectTitle -> ${title}`);
    state.projectTitle = title;
  }),

  setTrimStart: (time: number) => set((state) => {
    state.trimStart = time;
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  setTrimEnd: (time: number) => set((state) => {
    state.trimEnd = time;
    state.history.push({ ...state });
    state.historyIndex++;
  }),

  selectKeyframe: (keyframe: any) => set((state) => {
    state.selectedKeyframe = keyframe;
  }),

  undo: () => set((state) => {
    if (state.historyIndex > 0) {
      state.historyIndex--;
      const previousState = state.history[state.historyIndex];
      Object.assign(state, previousState);
    }
  }),

  redo: () => set((state) => {
    if (state.historyIndex < state.history.length - 1) {
      state.historyIndex++;
      const nextState = state.history[state.historyIndex];
      Object.assign(state, nextState);
    }
  }),
});
