import { describe, expect, it } from 'vitest';
import { exportTimeline } from './exportTimeline';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
const clip: TimelineClip = { id: 'v', name: 'Video', type: 'video', trackId: 't', startTime: 0,
  duration: 2000, sourceIn: 0, sourceOut: 1000, playbackRate: 0.5, enabled: true, locked: false };
const track: TimelineTrack = { id: 't', name: 'Video', type: 'video', clips: [], height: 40,
  visible: true, muted: false, solo: false, locked: false, order: 0 };
const state = { clips: [clip], tracks: [track], duration: 1000, trimStart: 0, trimEnd: 2000 };
describe('export timeline transport', () => {
  it('preserves source timing and the edited full-range trim for slow clips', () => {
    expect(exportTimeline(state)).toMatchObject({ trim_end: 2000,
      timeline_clips: [{ sourceIn: 0, sourceOut: 1000, playbackRate: 0.5, duration: 2000 }] });
    expect(exportTimeline({ ...state, trimEnd: 700 }).trim_end).toBe(700);
  });
  it('honors hidden, disabled and solo tracks without reverting to unedited source', () => {
    expect(exportTimeline({ ...state, tracks: [{ ...track, visible: false }] }).timeline_clips).toEqual([]);
    expect(exportTimeline({ ...state, clips: [{ ...clip, enabled: false }] }).timeline_clips).toEqual([]);
    const second = { ...track, id: 'solo', solo: true, muted: true, order: 1 };
    const result = exportTimeline({ ...state, tracks: [track, second], clips: [clip, { ...clip, id: 'second', trackId: 'solo' }] });
    expect(result.timeline_clips).toHaveLength(1);
    expect(result.timeline_clips?.[0]).toMatchObject({ id: 'second', muted: true, trackOrder: 1 });
  });
  it('does not export the original recording after every clip was deleted', () => {
    expect(() => exportTimeline({ ...state, clips: [] })).toThrow('nonempty');
    expect(exportTimeline({ ...state, tracks: [] })).toEqual({});
  });
});

it('transports cursor visibility without changing the source timing', () => {
  expect(exportTimeline({ ...state, clips: [{ ...clip, hideCursor: true }] }).timeline_clips?.[0])
    .toMatchObject({ hideCursor: true, sourceIn: 0, sourceOut: 1000, playbackRate: 0.5 });
  expect(exportTimeline(state).timeline_clips?.[0].hideCursor).toBe(false);
});

it('transports each clip audio channel without converting its linear gain', () => {
  expect(exportTimeline({ ...state, clips: [{ ...clip, micVolume: 0, systemVolume: 0.7 }] }).timeline_clips?.[0])
    .toMatchObject({ micVolume: 0, systemVolume: 0.7 });
  expect(exportTimeline(state).timeline_clips?.[0]).toMatchObject({ micVolume: 1, systemVolume: 1 });
});

it('preserves an explicit trim beyond source duration after slow motion', () => {
  expect(exportTimeline({ ...state, trimEnd: 1500 }).trim_end).toBe(1500);
});

it('serializes fractional speed-edited trims as nonempty integer-millisecond bounds', () => {
  const result=exportTimeline({...state,trimStart:1000/24,trimEnd:1001/24});
  expect(result.trim_start).toBe(41);
  expect(result.trim_end).toBe(42);
  expect(Number.isSafeInteger(result.trim_start)).toBe(true);
  expect(Number.isSafeInteger(result.trim_end)).toBe(true);
  expect(result.timeline_clips?.[0].sourceOut).toBe(1000);
});
