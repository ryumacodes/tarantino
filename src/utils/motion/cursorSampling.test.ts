import { expect, it } from 'vitest';
import { sampleCursorFrame } from './cursorSampling';

const frames = [
  [0, 0, 1, 0, 0, .2, .3, 0, 0, 0],
  [1, .5, 0, 1, .5, .8, .9, 20, 10, .1],
];

it('interpolates continuous cursor channels without anticipating click effects', () => {
  const actual = sampleCursorFrame(frames, 1000 / 240);
  [
    .5, .25, .5, 0, 0, .2, .3, 10, 5, .05,
  ].forEach((value, index) => expect(actual[index]).toBeCloseTo(value, 12));
  expect(actual.slice(3, 7)).toEqual(frames[0].slice(3, 7));
  expect(sampleCursorFrame(frames, 1000 / 120)).toEqual(frames[1]);
});

it('does not blend across clip boundaries or gaps', () => {
  for (const time of [3, 5, 7]) {
    expect(sampleCursorFrame(frames, time, [[0, 4], [6, 20]])).toEqual(frames[0]);
  }
  expect(sampleCursorFrame(frames, 1000 / 240, [[0, 20]])[0]).toBeCloseTo(.5, 12);
});

it('clamps samples at the available trajectory bounds', () => {
  expect(sampleCursorFrame(frames, -1)).toEqual(frames[0]);
  expect(sampleCursorFrame(frames, NaN)).toEqual(frames[0]);
  expect(sampleCursorFrame(frames, 1000)).toEqual(frames[1]);
});

it('retains spring overshoot until rendering so fractional fades stay linear', () => {
  const samples = [[0, 0, -.1, 0, 0, 0, 0, 0, -.2, 0], [0, 0, .3, 0, 0, 0, 0, 0, .6, 0]];
  const midpoint = sampleCursorFrame(samples, 1000 / 240);
  expect(midpoint[2]).toBeCloseTo(.1, 12);
  expect(midpoint[8]).toBeCloseTo(.2, 12);
  expect(sampleCursorFrame(samples, 0)[2]).toBe(-.1);
});
