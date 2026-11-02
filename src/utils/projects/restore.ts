import type { EditorState, EditorActions } from '../../stores/editor/types';
import type { ProjectDocument } from './document';

/** Restore saved edits while invalidating all work owned by the previous session. */
export function restoredProjectState(document: ProjectDocument, current: EditorState): Partial<EditorState & EditorActions> {
  return {
    ...document.state,
    recordingRevision: (current.recordingRevision ?? 0)+1,
    playbackRevision: current.playbackRevision+1,
    currentTime: 0,
    isPlaying: false,
    history: [], historyIndex: -1, historyGroup: null, canUndo: false, canRedo: false,
    selectedKeyframe: null, selectedBlockId: null,
    selection: {clipIds:[],trackIds:[],keyframeIds:[]},
    typingRanges: [],
    shortcutIntervals: [],
    mouseEvents: null, mouseEventsLoading: false,
    thumbnails: [], thumbnailsLoading: false,
    zoomLoading: false,
    previewZoomAnalysis: null, previewZoomLoading: false,
  };
}
