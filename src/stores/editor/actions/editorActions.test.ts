import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createClipsActions } from './clipsActions';
import { createPlaybackActions } from './playbackActions';
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

const harness = (state: any, factory: (set: any, get: any) => any) => {
  const set = (recipe: (draft: any) => void) => recipe(state);
  const actions = factory(set, () => state);
  Object.assign(state, actions);
  return actions;
};

beforeEach(() => {
  vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
});

describe('timeline clip actions', () => {
  it('cuts a clip while preserving its source mapping', () => {
    const original = clip('clip-a');
    const videoTrack = track('track-a');
    videoTrack.clips.push(original);
    const state: any = {
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
    const state: any = {
      clips: [original],
      tracks: [videoTrack],
      selection: { clipIds: [], trackIds: [], keyframeIds: [] },
      zoomKeyframes: [],
      currentTime: 0,
    };
    const actions = harness(state, createClipsActions);

    actions.setClipPlaybackRate('clip-a', 10);
    expect(original.playbackRate).toBe(4);
    expect(original.duration).toBe(250);
  });

  it('moves a clip between tracks without duplicating it', () => {
    const original = clip('clip-a');
    const first = track('track-a');
    const second = track('track-b');
    first.clips.push(original);
    const state: any = {
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

describe('zoom block actions', () => {
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

  it('clamps block end time and marks manual center edits', () => {
    const state: any = {
      duration: 5_000,
      zoomAnalysis: { zoom_blocks: [structuredClone(zoomBlock)] },
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
    });
  });

  it('splits centers and timing at the requested frame time', () => {
    const state: any = {
      duration: 5_000,
      zoomAnalysis: { zoom_blocks: [structuredClone(zoomBlock)] },
      history: [],
      historyIndex: -1,
      selectedBlockId: null,
    };
    const actions = harness(state, createZoomActions);

    actions.splitZoomBlocksAtTime(2_000);
    expect(state.zoomAnalysis.zoom_blocks).toHaveLength(2);
    expect(state.zoomAnalysis.zoom_blocks[0]).toMatchObject({
      id: 'zoom-a',
      start_time: 1_000,
      end_time: 2_000,
      centers: [{ x: 0.4, y: 0.4, time: 1_500 }],
    });
    expect(state.zoomAnalysis.zoom_blocks[1]).toMatchObject({
      id: 'generated-id',
      start_time: 2_000,
      end_time: 4_000,
      centers: [{ x: 0.6, y: 0.6, time: 3_000 }],
    });
  });
});

describe('playback actions', () => {
  it('updates playback state and records trim edits in history', () => {
    const state: any = {
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
    expect(state.historyIndex).toBe(1);
    expect(state.history).toHaveLength(2);
  });
});
