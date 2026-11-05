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

it('interpolates continuous style channels across cuts while suppressing gaps', () => {
  expect(sampleCursorFrame(frames, 5, [[0, 4], [4, 20]])[0]).toBeCloseTo(.6, 12);
  expect(sampleCursorFrame(frames, 5, [[0, 4], [6, 20]])).toEqual(Array(10).fill(0));
  expect(sampleCursorFrame(frames, 7, [[0, 4], [6, 20]])[0]).toBeCloseTo(.84, 12);
});

it('preserves sixty-hertz style interpolation around fractional cuts', () => {
  const cases = [
    [4, 3, .308], [4, 4, .344], [4, 5, .38],
    [8.5, 7.5, .47], [8.5, 8.5, .506], [8.5, 9.5, .542],
    [20, 19, .284], [20, 20, .32], [20, 21, .356],
    [503.25, 502.25, .281], [503.25, 503.25, .317], [503.25, 504.25, .353],
  ];
  for (const [cut, time, expected] of cases) {
    const packets = Array.from({length: 65}, (_, index) => {
      const lower = Math.floor(index / 2);
      const a = lower * 1000 / 60 < cut ? .2 : .8;
      const b = (lower + 1) * 1000 / 60 < cut ? .2 : .8;
      return [a + (b - a) * (index % 2) / 2, .5, 1, 0, 0, 0, 0, 0, 0, 0];
    });
    expect(sampleCursorFrame(packets, time, [[0, cut], [cut, 540]])[0]).toBeCloseTo(expected, 12);
  }
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
