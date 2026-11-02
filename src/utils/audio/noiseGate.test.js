import { expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { NoiseGate } from './noiseGate';

it('bypasses without changing samples and preserves stereo balance when gating', () => {
  const input = [new Float32Array(4800).fill(0.001), new Float32Array(4800).fill(-0.002)];
  const output = input.map(channel => new Float32Array(channel.length));
  const gate = new NoiseGate(48000);
  gate.process(input, output, false);
  expect(output).toEqual(input);
  gate.reset();
  gate.process(input, output, true);
  expect(output[0][4799]).toBeCloseTo(0.001 * 0.06125, 8);
  expect(output[1][4799]).toBeCloseTo(-0.002 * 0.06125, 8);
});

it('is independent of audio render block boundaries and resets on seek', () => {
  const input = Float32Array.from({ length: 5000 }, (_, n) => Math.sin(n / 17) * (n < 2000 ? 0.12 : 0.003));
  const whole = new Float32Array(input.length);
  new NoiseGate(44100).process([input], [whole], true);
  const gate = new NoiseGate(44100);
  const split = new Float32Array(input.length);
  for (let n = 0; n < input.length; n += 128) {
    gate.process([input.subarray(n, n + 128)], [split.subarray(n, n + 128)], true);
  }
  expect(split).toEqual(whole);
  gate.reset();
  gate.process([input], [split], true);
  expect(split).toEqual(whole);
});

it.skipIf(process.env.TARANTINO_AUDIO_INTEGRATION !== '1')('matches decoded FFmpeg gate samples at 44.1 and 48 kHz', () => {
  for (const rate of [44100, 48000]) {
    const input = [new Float32Array(rate), new Float32Array(rate)];
    const interleaved = new Float32Array(rate * 2);
    for (let n = 0; n < rate; n++) {
      const amplitude = [0, 0.001, 0.015, 0.08, 0.003][Math.floor(n * 5 / rate)];
      input[0][n] = amplitude * Math.sin(2 * Math.PI * 440 * n / rate);
      input[1][n] = amplitude * 0.6 * Math.sin(2 * Math.PI * 880 * n / rate);
      interleaved[n * 2] = input[0][n]; interleaved[n * 2 + 1] = input[1][n];
    }
    const result = spawnSync('ffmpeg', ['-v', 'error', '-f', 'f32le', '-ar', String(rate), '-ac', '2', '-i', 'pipe:0',
      '-af', 'agate=threshold=0.015:ratio=8:attack=5:release=80', '-f', 'f32le', 'pipe:1'],
    { input: Buffer.from(interleaved.buffer), maxBuffer: 4 * 1024 * 1024 });
    expect(result.status, result.stderr?.toString()).toBe(0);
    const expected = new Float32Array(result.stdout.buffer, result.stdout.byteOffset, result.stdout.length / 4);
    const actual = input.map(channel => new Float32Array(channel.length));
    new NoiseGate(rate).process(input, actual, true);
    let maxError = 0;
    for (let n = 0; n < rate; n++) for (let c = 0; c < 2; c++) {
      maxError = Math.max(maxError, Math.abs(actual[c][n] - expected[n * 2 + c]));
    }
    expect(expected.length).toBe(rate * 2);
    expect(maxError).toBeLessThan(1e-7);
  }
});
