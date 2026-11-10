import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorStore } from '../store';
import type { EditorState, EditorActions } from '../types';
import { createClipsActions } from './clipsActions';
import { createPlaybackActions } from './playbackActions';
import { splitTimelineAtTime } from '../../../utils/timeline/splitTimeline';
import { createZoomActions } from './zoomActions';

const clip = (id: string, trackId = 'track-a') => ({
  id,
  name: id,
  type: 'video' as const,
  trackId,
  startTime: 100,
  duration: 1_000,
  sourceIn: 50,
  sourceOut: 1_050,
  enabled: true,
  locked: false,
  playbackRate: 1,
});

const track = (id: string) => ({
  id,
  name: id,
  type: 'video' as const,
  clips: [] as ReturnType<typeof clip>[],
  height: 80,
  visible: true,
  muted: false,
  solo: false,
  locked: false,
  order: 0,
});

type Store = EditorState & EditorActions;
type Update = Partial<Store> | ((draft: Store) => void);
const harness = <T,>(state: Store, factory: (set: (update: Update) => void, get: () => Store) => T) => {
  const set = (update: Update) => typeof update === 'function' ? update(state) : Object.assign(state, update);
  const actions = factory(set, () => state);
  Object.assign(state, actions);
  return actions;
};

beforeEach(() => {
  vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
});

describe('timeline clip actions', () => {
  it.each([0.5, 2, 4])('preserves source time when splitting a %sx clip', (rate) => {
    const original = { ...clip('clip-a'), playbackRate: rate, duration: 1000 / rate };
    const videoTrack = track('track-a');
    videoTrack.clips.push(original);
    const state = { ...useEditorStore.getInitialState(), clips: [original], tracks: [videoTrack] };
    const actions = harness(state, createClipsActions);
    const midpoint = original.startTime + original.duration / 2;
    actions.cutClip(original.id, midpoint);
    expect(state.clips[0].sourceOut).toBe(550);
    expect(state.clips[1].sourceIn).toBe(550);
    expect(state.clips[1].sourceOut).toBe(1050);
    expect(state.clips[0].duration + state.clips[1].duration).toBe(1000 / rate);
  });
  it('cuts a clip while preserving its source mapping', () => {
    const original = clip('clip-a');
    const videoTrack = track('track-a');
    videoTrack.clips.push(original);
    const state = { ...useEditorStore.getInitialState(),
      clips: [original],
      tracks: [videoTrack],
      selection: { clipIds: [], trackIds: [], keyframeIds: [] },
      zoomKeyframes: [],
      currentTime: 0,
    };
    const actions = harness(state, createClipsActions);

    expect(actions.cutClip('clip-a', 600)).toBe('generated-id');
    expect(state.clips).toHaveLength(2);
    expect(state.clips[0]).toMatchObject({ duration: 500, sourceIn: 50, sourceOut: 550 });
    expect(state.clips[1]).toMatchObject({
      id: 'generated-id',
      startTime: 600,
      duration: 500,
      sourceIn: 550,
      sourceOut: 1_050,
    });
  });

  it('clamps playback rate and updates timeline duration', () => {
    const original = clip('clip-a');
    const videoTrack = track('track-a');
    videoTrack.clips.push(original);
    const state = { ...useEditorStore.getInitialState(),
      clips: [original],
      tracks: [videoTrack],
      selection: { clipIds: [], trackIds: [], keyframeIds: [] },
      zoomKeyframes: [],
      currentTime: 0,
    };
    const actions = harness(state, createClipsActions);

    actions.setClipPlaybackRate('clip-a', 100);
    expect(original.playbackRate).toBe(24);
    expect(original.duration).toBeCloseTo(1000 / 24);
  });

  it('moves a clip between tracks without duplicating it', () => {
    const original = clip('clip-a');
    const first = track('track-a');
    const second = track('track-b');
    first.clips.push(original);
    const state = { ...useEditorStore.getInitialState(),
      clips: [original],
      tracks: [first, second],
      selection: { clipIds: [], trackIds: [], keyframeIds: [] },
      zoomKeyframes: [],
      currentTime: 0,
    };
    const actions = harness(state, createClipsActions);

    actions.moveClip('clip-a', 500, 'track-b');
    expect(first.clips).toHaveLength(0);
    expect(second.clips).toEqual([original]);
    expect(original).toMatchObject({ trackId: 'track-b', startTime: 500 });
  });
});

  const zoomBlock = {
    id: 'zoom-a',
    click_x: 0.5,
    click_y: 0.5,
    center_x: 0.5,
    center_y: 0.5,
    start_time: 1_000,
    end_time: 4_000,
    zoom_factor: 2,
    is_manual: false,
    centers: [
      { x: 0.4, y: 0.4, time: 1_500 },
      { x: 0.6, y: 0.6, time: 3_000 },
    ],
  };

describe('zoom block actions', () => {


  it('clamps block end time and marks manual center edits', () => {
    const state = { ...useEditorStore.getInitialState(),
      duration: 5_000,
      zoomAnalysis: { total_clicks: 0, session_duration: 5000, config: {enabled:true,zoom_factor:2,zoom_duration:5000,min_click_spacing:500}, zoom_blocks: [structuredClone(zoomBlock)] },
      history: [],
      historyIndex: -1,
      selectedBlockId: null,
    };
    const actions = harness(state, createZoomActions);

    actions.updateZoomBlock('zoom-a', { center_x: 0.75, end_time: 9_000 });
    expect(state.zoomAnalysis.zoom_blocks[0]).toMatchObject({
      center_x: 0.75,
      end_time: 5_000,
      is_manual: true,
      timing_adjusted: true,
    });
  });

  it('returns a manually positioned zoom to automatic following', () => {
    const state = { ...useEditorStore.getInitialState(), duration: 5000, zoomAnalysis: { total_clicks: 0, session_duration: 5000, config: {enabled:true,zoom_factor:2,zoom_duration:5000,min_click_spacing:500}, zoom_blocks: [{ ...zoomBlock, is_manual: true }] }, history: [], historyIndex: -1 };
    const actions = harness(state, createZoomActions);
    actions.updateZoomBlock('zoom-a', { center_x: 0.5, center_y: 0.5, is_manual: false });
    expect(state.zoomAnalysis.zoom_blocks[0].is_manual).toBe(false);
  });

  it('splits centers and timing at the requested frame time', () => {
    const state = { ...useEditorStore.getInitialState(),
      duration: 5_000,
      zoomAnalysis: { total_clicks: 0, session_duration: 5000, config: {enabled:true,zoom_factor:2,zoom_duration:5000,min_click_spacing:500}, zoom_blocks: [structuredClone(zoomBlock)] },
      history: [],
      historyIndex: -1,
      selectedBlockId: null,
    };
    const actions = harness(state, createZoomActions);

    actions.splitZoomBlockAtTime('zoom-a', 2_000);
    expect(state.zoomAnalysis.zoom_blocks).toHaveLength(2);
    expect(state.zoomAnalysis.zoom_blocks[0]).toMatchObject({
      id: 'zoom-a',
      start_time: 1_000,
      end_time: 2_000,
      centers: [{ x: 0.4, y: 0.4, time: 1_500 }],
    });
    expect(state.zoomAnalysis.zoom_blocks[1]).toMatchObject({
      id: 'generated-id',
      is_manual: false,
      start_time: 2_000,
      end_time: 4_000,
      centers: [{ x: 0.6, y: 0.6, time: 3_000 }],
    });
  });
});

describe('playback actions', () => {
  it('updates playback state and records trim edits in history', () => {
    const state = { ...useEditorStore.getInitialState(),
      isPlaying: false,
      currentTime: 0,
      trimStart: 0,
      trimEnd: 10_000,
      duration: 10_000,
      history: [],
      historyIndex: -1,
    };
    const actions = harness(state, createPlaybackActions);

    actions.setIsPlaying(true);
    actions.setCurrentTime(2_500);
    actions.setTrimStart(500);
    actions.setTrimEnd(9_000);

    expect(state.isPlaying).toBe(true);
    expect(state.currentTime).toBe(2_500);
    expect(state.trimStart).toBe(500);
    expect(state.trimEnd).toBe(9_000);
    expect(state.historyIndex).toBe(2);
    expect(state.history).toHaveLength(3);
  });
});

describe('playback motion invalidation',()=>{
  it('distinguishes clock advancement from explicit seeks and playback transitions',()=>{
    const state={...useEditorStore.getInitialState(),isPlaying:false,currentTime:0,playbackRevision:0};
    const actions=harness(state,createPlaybackActions);
    actions.setIsPlaying(true);
    expect(state.playbackRevision).toBe(1);
    actions.setCurrentTime(16,'playback');actions.setCurrentTime(32,'playback');
    expect(state.playbackRevision).toBe(1);
    actions.setCurrentTime(33);
    expect(state.playbackRevision).toBe(2);
    actions.setIsPlaying(false);actions.setIsPlaying(false);
    expect(state.playbackRevision).toBe(3);
    actions.setIsPlaying(true);
    expect(state.playbackRevision).toBe(4);
  });
});


describe('splitting the targeted timeline track', () => {
  const setup = () => {
    const video = { ...clip('video'), startTime: 0, duration: 2000,
      sourceIn: 1000, sourceOut: 5000, playbackRate: 2 };
    const videoTrack = { ...track('track-a'), clips: [video] };
    const state = { ...useEditorStore.getInitialState(), duration: 5000,
      clips: [video], tracks: [videoTrack], history: [], historyIndex: -1,
      zoomAnalysis: { total_clicks: 0, session_duration: 5000,
        config: { enabled: true, zoom_factor: 2, zoom_duration: 5000, min_click_spacing: 500 },
        zoom_blocks: [structuredClone(zoomBlock), { ...structuredClone(zoomBlock), id: 'other' }] },
    };
    harness(state, createClipsActions);
    harness(state, createZoomActions);
    return state;
  };

  it('cuts video without changing a zoom spanning the cut', () => {
    const state = setup();
    const zooms = structuredClone(state.zoomAnalysis);
    expect(splitTimelineAtTime(state, 500, null)).toEqual(['generated-id']);
    expect(state.clips).toHaveLength(2);
    expect(state.clips.map(c => [c.sourceIn, c.sourceOut])).toEqual([[1000, 2000], [2000, 5000]]);
    expect(state.zoomAnalysis).toEqual(zooms);
  });

  it('splits only the selected zoom using edited source time', () => {
    const state = setup();
    const clips = structuredClone(state.clips);
    const other = structuredClone(state.zoomAnalysis.zoom_blocks[1]);
    expect(splitTimelineAtTime(state, 500, 'zoom-a')).toEqual([]);
    expect(state.clips).toEqual(clips);
    expect(state.zoomAnalysis.zoom_blocks).toHaveLength(3);
    expect(state.zoomAnalysis.zoom_blocks.find(b => b.id === 'other')).toEqual(other);
    expect(state.zoomAnalysis.zoom_blocks.find(b => b.id === 'zoom-a')?.end_time).toBe(2000);
    expect(state.zoomAnalysis.zoom_blocks.find(b => b.id === 'generated-id')?.start_time).toBe(2000);
  });

  it('does not fall back to cutting video when outside the selected zoom', () => {
    const state = setup();
    const clips = structuredClone(state.clips);
    const zooms = structuredClone(state.zoomAnalysis);
    splitTimelineAtTime(state, 1750, 'zoom-a');
    expect(state.clips).toEqual(clips);
    expect(state.zoomAnalysis).toEqual(zooms);
  });
});
