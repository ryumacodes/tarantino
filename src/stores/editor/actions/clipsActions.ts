import { typingSpeedSuggestions } from '../../../utils/timeline/typingSpeedSuggestions';
import { trimClipEdge } from '../../../utils/timeline/trimClip';
import { restoredClipBounds, type TrimSide } from '../../../utils/timeline/resetClipTrim';
import { removablePreviousClip } from '../../../utils/timeline/previousClip';
import { pruneZoomRanges } from '../../../utils/timeline/pruneZoomRanges';
import { mergeNeighbor, type MergeDirection } from '../../../utils/timeline/mergeClips';
import { edit } from '../history';

import type {
  EditorState,
  EditorActions,
  TimelineTool,
  TimelineTrack,
  TimelineClip,
  TimelineViewState,
  SnappingTarget,
} from '../types';

type SetFn = (fn: (state: EditorState & EditorActions) => void) => void;
type GetFn = () => EditorState & EditorActions;

function speedUpTypingRange(state: EditorState & EditorActions, clipId: string, from: number, to: number, rate: number) {
    const clip = state.clips.find(c => c.id === clipId);
    const track = state.tracks.find(t => t.id === clip?.trackId);
    if (!clip || !track || clip.locked || track.locked || !clip.enabled || !track.visible
      || (clip.sourceFilePath && clip.sourceFilePath !== state.videoFilePath)
      || !['video','screen'].includes(clip.type) || ![from,to,rate].every(Number.isFinite)
      || rate < 1.5 || rate > 8 || from < clip.sourceIn || to > clip.sourceOut || to <= from) return;
    const range = state.typingRanges.find(range => range.start <= from && range.end >= to);
    if (range && !state.dismissedTypingRanges.includes(range.start))
      state.dismissedTypingRanges.push(range.start);
    if (clip.playbackRate === rate) return;
    const original = {...clip};
    const pieces: TimelineClip[] = [];
    const piece = (start: number, end: number, id: string) => ({...original,id,sourceIn:start,sourceOut:end,
      startTime:original.startTime+(start-original.sourceIn)/original.playbackRate,
      duration:(end-start)/original.playbackRate});
    if (from > original.sourceIn) pieces.push(piece(original.sourceIn,from,crypto.randomUUID()));
    pieces.push(piece(from,to,original.id));
    if (to < original.sourceOut) pieces.push(piece(to,original.sourceOut,crypto.randomUUID()));
    state.clips = state.clips.flatMap(c => c.id === clipId ? pieces : [c]);
    track.clips = state.clips.filter(c => c.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
    changeClipSpeed(state, original.id, rate);
    state.selection.clipIds = [original.id];
}

// Shared by individual and batch edits so timeline remapping stays identical.
function changeClipSpeed(state: EditorState & EditorActions, clipId: string, rate: number) {
  const clip = state.clips.find(c => c.id === clipId);
  if (!clip || clip.locked || !Number.isFinite(rate)) return;
  const track = state.tracks.find(t => t.id === clip.trackId);
  if (track?.locked) return;
  const newRate = Math.max(0.25, Math.min(24, rate));
  if (newRate === clip.playbackRate) return;
  const oldDuration = clip.duration;
  if (oldDuration <= 0 || clip.sourceOut <= clip.sourceIn) return;
  const oldEnd = clip.startTime + oldDuration;
  const newDuration = (clip.sourceOut - clip.sourceIn) / newRate;
  const delta = newDuration - oldDuration;
  const remap = (time: number) => time <= clip.startTime ? time
    : time >= oldEnd ? time + delta
    : clip.startTime + (time - clip.startTime) * newDuration / oldDuration;
  // Keep the same source moments selected when the edited clock stretches.
  if (clip.type === 'video' || clip.type === 'screen') {
    state.trimStart = remap(state.trimStart);
    state.trimEnd = remap(state.trimEnd);
    state.currentTime = remap(state.currentTime);
    state.playbackRevision = state.playbackRevision + 1;
  }
  for (const following of state.clips) {
    if (following.id !== clip.id && following.trackId === clip.trackId && following.startTime >= oldEnd) {
      following.startTime += delta;
    }
  }
  clip.playbackRate = newRate;
  clip.duration = newDuration;
  if (track) {
    track.clips = state.clips.filter(c => c.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
  }
}

export const createClipsActions = (set: SetFn, get: GetFn) => ({
  setCurrentTool: (tool: TimelineTool) => set((state) => {
    state.currentTool = tool;
  }),

  addTrack: (track: TimelineTrack) => edit(set, (state) => {
    state.tracks.push(track);
    state.tracks.sort((a, b) => a.order - b.order);
  }),

  updateTrack: (id: string, updates: Partial<TimelineTrack>) => edit(set, (state) => {
    const track = state.tracks.find(t => t.id === id);
    if (track) {
      Object.assign(track, updates);
    }
  }),

  setVideoTracksVisible: (visible: boolean) => edit(set, (state) => {
    for (const track of state.tracks) {
      if (track.type === 'video' || track.type === 'screen') track.visible = visible;
    }
  }),

  deleteTrack: (id: string) => edit(set, (state) => {
    state.tracks = state.tracks.filter(t => t.id !== id);
    state.clips = state.clips.filter(c => c.trackId !== id);
  }),

  addClip: (clip: TimelineClip) => edit(set, (state) => {
    state.clips.push(clip);
    const track = state.tracks.find(t => t.id === clip.trackId);
    if (track) {
      track.clips.push(clip);
      track.clips.sort((a, b) => a.startTime - b.startTime);
    }
  }),

  updateClip: (id: string, updates: Partial<TimelineClip>) => edit(set, (state) => {
    const clip = state.clips.find(c => c.id === id);
    if (clip) {
      Object.assign(clip, updates);
      const track = state.tracks.find(t => t.id === clip.trackId);
      if (track) {
        const trackClip = track.clips.find(c => c.id === id);
        if (trackClip) {
          Object.assign(trackClip, updates);
        }
        track.clips.sort((a, b) => a.startTime - b.startTime);
      }
    }
  }),

  deleteClip: (id: string) => edit(set, (state) => {
    const clip = state.clips.find(c => c.id === id);
    if (!clip || clip.locked) return;
    const track = state.tracks.find(t => t.id === clip.trackId);
    if (track?.locked) return;
    const isVideo = clip.type === 'video' || clip.type === 'screen';
    if (isVideo && state.clips.filter(c => c.trackId === clip.trackId).length <= 1) return;
    if (isVideo) {
      const end = clip.startTime+clip.duration;
      const remap = (time: number) => time <= clip.startTime ? time
        : time >= end ? time-clip.duration : clip.startTime;
      state.trimStart = remap(state.trimStart);
      state.trimEnd = remap(state.trimEnd);
      state.currentTime = remap(state.currentTime);
      state.playbackRevision = state.playbackRevision+1;
      for (const following of state.clips) {
        if (following.trackId === clip.trackId && following.id !== id && following.startTime >= end) {
          following.startTime -= clip.duration;
        }
      }
    }
    state.clips = state.clips.filter(c => c.id !== id);
    if (track) track.clips = state.clips.filter(c => c.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
    state.selection.clipIds = state.selection.clipIds.filter(cId => cId !== id);
    if (isVideo) pruneZoomRanges(state);
    // If the whole selected export range was deleted, select the remaining video.
    if (isVideo && state.trimEnd <= state.trimStart) {
      const remaining = state.clips.filter(c => c.type === 'video' || c.type === 'screen');
      state.trimStart = Math.min(...remaining.map(c => c.startTime));
      state.trimEnd = Math.max(...remaining.map(c => c.startTime+c.duration));
    }
  }),

  removePreviousClip: () => {
    const state = get();
    const previous = removablePreviousClip(state, state.currentTime);
    if (previous) state.deleteClip(previous.id);
  },

  cutClip: (clipId: string, time: number) => {
    let newClipId: string | null = null;

    edit(set, (state) => {
      const clip = state.clips.find(c => c.id === clipId);
      if (clip && time > clip.startTime && time < clip.startTime + clip.duration) {
        newClipId = crypto.randomUUID();
        const newClip: TimelineClip = {
          ...clip,
          id: newClipId,
          name: clip.name + ' (Part 2)',
          startTime: time,
          duration: clip.startTime + clip.duration - time,
          sourceIn: clip.sourceIn + (time - clip.startTime) * clip.playbackRate
        };

        clip.duration = time - clip.startTime;
        clip.sourceOut = clip.sourceIn + clip.duration * clip.playbackRate;

        state.clips.push(newClip);

        const track = state.tracks.find(t => t.id === clip.trackId);
        if (track) {
          const trackClip = track.clips.find(c => c.id === clipId);
          if (trackClip) {
            Object.assign(trackClip, clip);
          }
          track.clips.push(newClip);
          track.clips.sort((a, b) => a.startTime - b.startTime);
        }
      }
    });

    return newClipId;
  },

  mergeClip: (clipId: string, direction: MergeDirection) => edit(set, (state) => {
    const pair = mergeNeighbor(state.clips, state.tracks, clipId, direction);
    if (!pair) return;
    const { selected, neighbor, first, last } = pair;
    const start = first.startTime;
    const oldEnd = last.startTime + last.duration;
    const sourceStart = first.sourceIn;
    const sourceEnd = last.sourceOut;
    const rate = selected.playbackRate;
    const duration = (sourceEnd-sourceStart)/rate;
    const delta = start+duration-oldEnd;
    const remap = (time: number) => {
      if (time <= start) return time;
      if (time >= oldEnd) return time+delta;
      let source: number;
      if (time < first.startTime+first.duration) source = first.sourceIn+(time-first.startTime)*first.playbackRate;
      else if (time >= last.startTime) source = last.sourceIn+(time-last.startTime)*last.playbackRate;
      else source = first.sourceOut+(last.sourceIn-first.sourceOut)
        *(time-first.startTime-first.duration)/(last.startTime-first.startTime-first.duration);
      return start+(source-sourceStart)/rate;
    };
    if (selected.type === 'video' || selected.type === 'screen') {
      state.trimStart = remap(state.trimStart);
      state.trimEnd = remap(state.trimEnd);
      state.currentTime = remap(state.currentTime);
      state.playbackRevision = state.playbackRevision+1;
    }
    for (const clip of state.clips) {
      if (clip.trackId === selected.trackId && clip.id !== selected.id && clip.id !== neighbor.id && clip.startTime >= oldEnd) {
        clip.startTime += delta;
      }
    }
    selected.startTime = start;
    selected.sourceIn = sourceStart;
    selected.sourceOut = sourceEnd;
    selected.duration = duration;
    state.clips = state.clips.filter(clip => clip.id !== neighbor.id);
    const track = state.tracks.find(track => track.id === selected.trackId);
    if (track) track.clips = state.clips.filter(clip => clip.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
    state.selection.clipIds = [...new Set(state.selection.clipIds.map(id => id === neighbor.id ? selected.id : id))];
  }),

  trimClip: (clipId: string, side: TrimSide, sourceTime: number) => edit(set, state => {
    trimClipEdge(state, clipId, side, sourceTime);
  }),

  resetClipTrim: (clipId: string, side?: TrimSide) => edit(set, (state) => {
    const bounds = restoredClipBounds(state.clips, state.tracks, clipId, state.duration, state.videoFilePath, side);
    if (!bounds) return;
    const { clip, start, end } = bounds;
    const oldEnd = clip.startTime + clip.duration;
    const leading = (clip.sourceIn-start)/clip.playbackRate;
    const duration = (end-start)/clip.playbackRate;
    const delta = duration-clip.duration;
    const remap = (time: number) => time < clip.startTime ? time
      : time >= oldEnd ? time+delta : time+leading;
    state.currentTime = remap(state.currentTime);
    // An export boundary at the clip edge includes the newly restored footage.
    state.trimStart = state.trimStart === clip.startTime ? state.trimStart : remap(state.trimStart);
    state.trimEnd = remap(state.trimEnd);
    for (const following of state.clips) {
      if (following.id !== clip.id && following.trackId === clip.trackId && following.startTime >= oldEnd) {
        following.startTime += delta;
      }
    }
    clip.sourceIn = start;
    clip.sourceOut = end;
    clip.duration = duration;
    const track = state.tracks.find(track => track.id === clip.trackId);
    if (track) track.clips = state.clips.filter(item => item.trackId === track.id).sort((a,b) => a.startTime-b.startTime);
    state.playbackRevision = state.playbackRevision+1;
  }),

  getClipsAtTime: (time: number, trackId?: string) => {
    const state = get();
    return state.clips.filter(clip => {
      const clipEnd = clip.startTime + clip.duration;
      const withinClip = time > clip.startTime && time < clipEnd;
      const matchesTrack = trackId ? clip.trackId === trackId : true;
      return withinClip && matchesTrack;
    });
  },

  cutClipsAtTime: (time: number, trackId?: string) => {
    const clipsAtTime = get().getClipsAtTime(time, trackId);
    get().beginHistoryGroup?.();
    try {
      return clipsAtTime.map(clip => get().cutClip(clip.id, time))
        .filter((clipId): clipId is string => Boolean(clipId));
    } finally { get().endHistoryGroup?.(); }
  },

  speedUpTypingRange: (clipId: string, from: number, to: number, rate: number) => edit(set, state => {
    speedUpTypingRange(state, clipId, from, to, rate);
  }),

  speedUpAllTypingRanges: (rate: number) => edit(set, state => {
    // Later edits cannot shift the original start of an earlier clip or range.
    const suggestions = typingSpeedSuggestions(state).sort((a,b) =>
      b.clip.startTime-a.clip.startTime || b.from-a.from);
    for (const suggestion of suggestions) {
      const {clip: original, from, to} = suggestion;
      const clip = state.clips.find(candidate => candidate.trackId === original.trackId
        && candidate.startTime === original.startTime && candidate.sourceIn === original.sourceIn
        && candidate.sourceOut >= to);
      if (clip) speedUpTypingRange(state, clip.id, from, to, rate);
    }
  }),

  dismissTypingRange: (start: number) => edit(set, state => {
    if (Number.isFinite(start) && start >= 0 && !state.dismissedTypingRanges.includes(start))
      state.dismissedTypingRanges.push(start);
  }),

  setClipPlaybackRate: (clipId: string, rate: number) => edit(set, (state) => {
    changeClipSpeed(state, clipId, rate);
  }),

  applyClipSpeedToTrack: (clipId: string) => edit(set, (state) => {
    const source = state.clips.find(clip => clip.id === clipId);
    if (!source || source.locked || state.tracks.find(track => track.id === source.trackId)?.locked) return;
    // Snapshot IDs before rippling positions, then edit in playback order.
    const targets = state.clips.filter(clip => clip.trackId === source.trackId)
      .sort((a, b) => a.startTime - b.startTime).map(clip => clip.id);
    const rate = source.playbackRate;
    for (const id of targets) changeClipSpeed(state, id, rate);
  }),

  moveClip: (clipId: string, newStartTime: number, newTrackId?: string) => edit(set, (state) => {
    const clip = state.clips.find(c => c.id === clipId);
    if (clip) {
      const oldTrackId = clip.trackId;

      clip.startTime = newStartTime;
      if (newTrackId) {
        clip.trackId = newTrackId;
      }

      const oldTrack = state.tracks.find(t => t.id === oldTrackId);
      if (oldTrack) {
        oldTrack.clips = oldTrack.clips.filter(c => c.id !== clipId);
      }

      const newTrack = state.tracks.find(t => t.id === clip.trackId);
      if (newTrack) {
        const existingClip = newTrack.clips.find(c => c.id === clipId);
        if (!existingClip) {
          newTrack.clips.push(clip);
        }
        newTrack.clips.sort((a, b) => a.startTime - b.startTime);
      }
    }
  }),

  selectClips: (clipIds: string[], addToSelection = false) => set((state) => {
    if (addToSelection) {
      const newIds = clipIds.filter(id => !state.selection.clipIds.includes(id));
      state.selection.clipIds.push(...newIds);
    } else {
      state.selection.clipIds = clipIds;
    }
  }),

  clearSelection: () => set((state) => {
    state.selection = {
      clipIds: [],
      trackIds: [],
      keyframeIds: []
    };
  }),

  setSnappingEnabled: (enabled: boolean) => set((state) => {
    state.snappingEnabled = enabled;
  }),

  updateSnappingTargets: () => set((state) => {
    const targets: SnappingTarget[] = [];

    state.clips.forEach(clip => {
      targets.push({
        time: clip.startTime,
        type: 'clip-start',
        id: clip.id
      });
      targets.push({
        time: clip.startTime + clip.duration,
        type: 'clip-end',
        id: clip.id
      });
    });

    state.zoomKeyframes.forEach(kf => {
      targets.push({
        time: kf.time,
        type: 'keyframe',
        id: kf.id
      });
    });

    targets.push({
      time: state.currentTime,
      type: 'playhead',
      id: 'playhead'
    });

    state.snappingTargets = targets;
  }),

  setViewState: (updates: Partial<TimelineViewState>) => set((state) => {
    Object.assign(state.viewState, updates);
  }),
});
