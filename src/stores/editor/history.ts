import { current, isDraft, original } from 'immer';
import type { EditorActions, EditorState } from './types';

type Store = EditorState & EditorActions;
type Setter = (recipe: (state: Store) => void) => void;
const fields = ['projectTitle', 'trimStart', 'trimEnd', 'clips', 'tracks', 'zoomKeyframes',
  'webcamKeyframes', 'masks', 'transcripts', 'captionSettings', 'shortcutSettings', 'dismissedTypingRanges', 'zoomAnalysis', 'overlays', 'audioSettings', 'visualSettings', 'exportSettings'] as const;
export type EditSnapshot = Pick<EditorState, typeof fields[number]>;
const LIMIT = 100;

function snapshot(state: Store): EditSnapshot {
  return Object.fromEntries(fields.map(key => [key, state[key]])) as EditSnapshot;
}

/** Store only immutable editable fields, never runtime caches, actions or history. */
export function edit(set: Setter, recipe: (state: Store) => void) {
  set(state => {
    const drafted = isDraft(state);
    const before = snapshot(drafted ? original(state)! : structuredClone(snapshot(state)) as Store);
    recipe(state);
    const after = snapshot(drafted ? current(state) : structuredClone(snapshot(state)) as Store);
    if (fields.every(key => before[key] === after[key])) return;
    if (state.historyGroup?.recorded) {
      state.history[state.historyIndex] = after;
      return;
    }
    const history = (state.history ?? []).slice(0, (state.historyIndex ?? -1) + 1);
    // Capture the immediate pre-edit state, including newly loaded analysis.
    if (!history.length) history.push(before);
    else history[history.length - 1] = before;
    history.push(after);
    state.history = history.slice(-(LIMIT + 1));
    state.historyIndex = state.history.length - 1;
    if (state.historyGroup) state.historyGroup.recorded = true;
    state.canUndo = true;
    state.canRedo = false;
  });
}

export function restoreEdit(state: Store, direction: -1 | 1) {
  state.historyGroup = null;
  const index = state.historyIndex + direction;
  if (index < 0 || index >= state.history.length) return;
  Object.assign(state, state.history[index]);
  state.historyIndex = index;
  state.canUndo = index > 0;
  state.canRedo = index < state.history.length - 1;
  state.isPlaying = false;
  state.playbackRevision = state.playbackRevision + 1;
  state.selection = { clipIds: [], trackIds: [], keyframeIds: [] };
  state.selectedKeyframe = null;
  state.selectedBlockId = null;
}
