// Clips Actions
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

export const createClipsActions = (set: SetFn, get: GetFn) => ({
  setCurrentTool: (tool: TimelineTool) => set((state) => {
    state.currentTool = tool;
  }),

  addTrack: (track: TimelineTrack) => set((state) => {
    state.tracks.push(track);
    state.tracks.sort((a, b) => a.order - b.order);
  }),

  updateTrack: (id: string, updates: Partial<TimelineTrack>) => set((state) => {
    const track = state.tracks.find(t => t.id === id);
    if (track) {
      Object.assign(track, updates);
    }
  }),

  deleteTrack: (id: string) => set((state) => {
    state.tracks = state.tracks.filter(t => t.id !== id);
    state.clips = state.clips.filter(c => c.trackId !== id);
  }),

  addClip: (clip: TimelineClip) => set((state) => {
    state.clips.push(clip);
    const track = state.tracks.find(t => t.id === clip.trackId);
    if (track) {
      track.clips.push(clip);
      track.clips.sort((a, b) => a.startTime - b.startTime);
    }
  }),

  updateClip: (id: string, updates: Partial<TimelineClip>) => set((state) => {
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

  deleteClip: (id: string) => set((state) => {
    const clip = state.clips.find(c => c.id === id);
    if (clip) {
      state.clips = state.clips.filter(c => c.id !== id);
      const track = state.tracks.find(t => t.id === clip.trackId);
      if (track) {
        track.clips = track.clips.filter(c => c.id !== id);
      }
      state.selection.clipIds = state.selection.clipIds.filter(cId => cId !== id);
    }
  }),

  cutClip: (clipId: string, time: number) => set((state) => {
    const clip = state.clips.find(c => c.id === clipId);
    if (clip && time > clip.startTime && time < clip.startTime + clip.duration) {
      const newClip: TimelineClip = {
        ...clip,
        id: crypto.randomUUID(),
        name: clip.name + ' (Part 2)',
        startTime: time,
        duration: clip.startTime + clip.duration - time,
        sourceIn: clip.sourceIn + (time - clip.startTime)
      };

      clip.duration = time - clip.startTime;
      clip.sourceOut = clip.sourceIn + clip.duration;

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
    console.log('cutClipsAtTime: Found', clipsAtTime.length, 'clips at time', time);
    clipsAtTime.forEach(clip => {
      get().cutClip(clip.id, time);
    });
  },

  setClipPlaybackRate: (clipId: string, rate: number) => set((state) => {
    const clip = state.clips.find(c => c.id === clipId);
    if (clip) {
      const sourceDuration = clip.sourceOut - clip.sourceIn;
      const oldRate = clip.playbackRate;
      const newRate = Math.max(0.25, Math.min(4, rate));
      clip.playbackRate = newRate;
      clip.duration = sourceDuration / newRate;

      const track = state.tracks.find(t => t.id === clip.trackId);
      if (track) {
        const trackClip = track.clips.find(c => c.id === clipId);
        if (trackClip) {
          trackClip.playbackRate = newRate;
          trackClip.duration = clip.duration;
        }
      }

      console.log(`Clip ${clipId}: playback rate ${oldRate}x -> ${newRate}x`);
    }
  }),

  moveClip: (clipId: string, newStartTime: number, newTrackId?: string) => set((state) => {
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
