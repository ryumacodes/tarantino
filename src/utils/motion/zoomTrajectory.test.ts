import { expect, it } from 'vitest';
import { sampleCameraFrame, cameraFrameIndex, type CameraFrame } from './zoomTrajectory';

it('returns identical frames when scrubbing backwards, replaying, or seeking directly', () => {
  const frames: CameraFrame[] = Array.from({ length: 240 }, (_, i) => [1 + i / 240, i / 240, 0.5]);
  const expected = sampleCameraFrame(frames, 750);
  for (const time of [0, 1800, 750, 200, 750, 0, 750]) {
    const result = sampleCameraFrame(frames, time);
    if (time === 750) expect(result).toEqual(expected);
  }
  expect(expected).toEqual(frames[90]);
});

it('interpolates fractional times and clamps to available timeline data', () => {
  const frames: CameraFrame[] = [[1, 0.5, 0.5], [3, 0.2, 0.2], [1, 0.5, 0.5]];
  expect(sampleCameraFrame(frames, 8)[0]).toBeCloseTo(2.92);
  expect(sampleCameraFrame(frames, 1000 / 120)).toEqual(frames[1]);
  expect(sampleCameraFrame(frames, 2000 / 120)).toEqual(frames[2]);
  expect(sampleCameraFrame(frames, -100)).toEqual(frames[0]);
  expect(sampleCameraFrame(frames, 10000)).toEqual(frames[2]);
  expect(sampleCameraFrame([], 100)).toEqual([1, 0.5, 0.5]);
  expect(cameraFrameIndex(NaN, 100)).toBe(0);
});

it('preserves sub-millisecond range boundaries and caret samples', async () => {
  const { exportZoomBlock } = await import('./zoomTrajectory');
  const result=exportZoomBlock({id:'fractional',start_time:123.4,end_time:999.8,zoom_factor:2,
    click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,is_manual:true,
    centers:[{time:500.6,x:0.4,y:0.3}]});
  expect(result).toMatchObject({start_time_ms:123.4,end_time_ms:999.8,centers:[{time:500.6}]});
});

it('sends bounded anticipation to the shared native preview/export representation', async () => {
  const { exportZoomBlock } = await import('./zoomTrajectory');
  const block = { id: 'anticipate', start_time: 1000, end_time: 2000, zoom_factor: 2,
    click_x: 0.5, click_y: 0.5, center_x: 0.5, center_y: 0.5, is_manual: true };
  expect(exportZoomBlock(block).animation_lead_in_ms).toBe(0);
  expect(exportZoomBlock(block, 650).animation_lead_in_ms).toBe(650);
  for (const invalid of [-10, NaN, Infinity]) expect(exportZoomBlock(block, invalid).animation_lead_in_ms).toBe(0);
  expect(exportZoomBlock(block, 5000).animation_lead_in_ms).toBe(2000);
});

it('transports disabled zooms identically for preview and export', async () => {
  const { exportZoomBlock } = await import('./zoomTrajectory');
  const block = {id:'toggle',start_time:100,end_time:900,zoom_factor:2,
    center_x:0.5,center_y:0.5,click_x:0.5,click_y:0.5,is_manual:true};
  expect(exportZoomBlock(block).is_disabled).toBe(false);
  expect(exportZoomBlock({...block,is_disabled:true}).is_disabled).toBe(true);
});

it('preserves independent magnifier style in the native request', async () => {
  const { exportZoomBlock } = await import('./zoomTrajectory');
  const block = {id:'lens',start_time:0,end_time:900,zoom_factor:2,
    center_x:0.5,center_y:0.5,click_x:0.5,click_y:0.5,is_manual:true};
  const loupe = {radius:0.25,glass:true,bevel:0.1,chromatic_aberration:0.01,shadow_alpha:0.35};
  expect(exportZoomBlock({...block,loupe}).loupe).toEqual(loupe);
  expect(exportZoomBlock(block).loupe).toBeNull();
});

it('interpolates every rectangle component at trimmed and speed-adjusted source times', () => {
  const frames: CameraFrame[] = [[1, .5, .5, 0, 0, 1, 1], [2, .75, .25, -.5, -.25, 2, 1.5]];
  for (const time of [1, 3.125, 6.25]) {
    const fraction = time * 120 / 1000;
    const expected = frames[0].map((value, i) => value! + (frames[1][i]! - value!) * fraction);
    const actual = sampleCameraFrame(frames, time);
    expect(actual.slice(3)).toEqual(expected.slice(3));
    expect(actual[0]).toBe(expected[0]);
    for (const [center, origin, extent] of [[1, 3, 5], [2, 4, 6]]) {
      const anchor = frames.map(frame => frame[origin]! + frame[center]! * frame[extent]!);
      expect(actual[origin]! + actual[center]! * actual[extent]!).toBeCloseTo(anchor[0] + (anchor[1] - anchor[0]) * fraction, 12);
    }
  }
});

it('interpolates display camera geometry before deriving its crop center', () => {
  const frames: CameraFrame[] = [[1, .5, .5, 0, 0, 1, 1], [2, .75, .75, -1, -1, 2, 2]];
  const frame = sampleCameraFrame(frames, 1000 / 240);
  expect(frame[0]).toBe(1.5);
  expect(frame[1]).toBeCloseTo(2 / 3, 12);
  expect(frame[2]).toBeCloseTo(2 / 3, 12);
  frame.slice(3).forEach((value, index) => expect(value).toBeCloseTo([-.5, -.5, 1.5, 1.5][index], 12));
});
