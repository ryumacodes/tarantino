import { beforeEach, expect, it, vi } from 'vitest';
import { channelGain, createPreviewAudio } from './previewAudio';
import type { AudioSettings } from '../../stores/editor/types';
const settings: AudioSettings = { micGain: 20, systemGain: -6, noiseGate: false, dualTrack: false };

beforeEach(() => {
  vi.stubGlobal('AudioWorkletNode', class {
    connect = vi.fn(); disconnect = vi.fn();
    port = { postMessage: vi.fn(), close: vi.fn() };
  });
});
function fixture() {
  const sources: any[] = [];
  const gains: any[] = [];
  const context = {
    audioWorklet: { addModule: vi.fn(async () => {}) },
    currentTime: 1, state: 'suspended', destination: {},
    createMediaElementSource: vi.fn(() => {
      const source = { connect: vi.fn(), disconnect: vi.fn() }; sources.push(source); return source;
    }),
    createGain: vi.fn(() => {
      const gain = { connect: vi.fn(), disconnect: vi.fn(), gain: { setValueAtTime: vi.fn(), setTargetAtTime: vi.fn() } };
      gains.push(gain); return gain;
    }),
    resume: vi.fn(async () => {}), suspend: vi.fn(async () => {}), close: vi.fn(async () => {}),
  };
  return { context, sources, gains };
}

it('preserves amplification, attenuation, and independent mute without losing the gain setting', () => {
  expect(channelGain(settings, 'microphone')).toBe(10);
  expect(channelGain(settings, 'system')).toBeCloseTo(0.501187, 6);
  expect(channelGain({ ...settings, micMuted: true }, 'microphone')).toBe(0);
  expect(channelGain({ ...settings, micMuted: true }, 'system')).toBeCloseTo(0.501187, 6);
  expect(channelGain({ ...settings, systemMuted: true }, 'system')).toBe(0);
  expect(channelGain({ ...settings, micGain: NaN }, 'microphone')).toBe(1);
});

it('updates levels without reattaching media sources and releases the graph exactly once', async () => {
  const { context, sources, gains } = fixture();
  const mic = { volume: 0.3, muted: true, addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as HTMLAudioElement;
  const system = { volume: 0.5, muted: false } as HTMLAudioElement;
  const graph = createPreviewAudio([{ element: mic, kind: 'microphone' }, { element: system, kind: 'system' }], settings, context as unknown as AudioContext);
  expect(mic.volume).toBe(1);
  expect(gains[0].gain.setValueAtTime).toHaveBeenCalledWith(10, 1);
  graph.update({ ...settings, micMuted: true });
  expect(gains[0].gain.setTargetAtTime.mock.lastCall[0]).toBe(0);
  graph.update(settings);
  expect(gains[0].gain.setTargetAtTime.mock.lastCall[0]).toBe(10);
  expect(context.createMediaElementSource).toHaveBeenCalledTimes(2);
  await graph.setPlaying(true);
  await graph.setPlaying(false);
  expect(context.resume).toHaveBeenCalledTimes(1);
  expect(context.suspend).toHaveBeenCalledTimes(1);
  graph.dispose(); graph.dispose();
  await graph.setPlaying(true);
  expect(context.close).toHaveBeenCalledTimes(1);
  expect(context.resume).toHaveBeenCalledTimes(1);
  for (const node of [...sources, ...gains]) expect(node.disconnect).toHaveBeenCalledTimes(1);
});

it('releases partially attached sources when initialization fails', () => {
  const { context, sources, gains } = fixture();
  const original = context.createMediaElementSource.getMockImplementation()!;
  context.createMediaElementSource.mockImplementationOnce(original).mockImplementationOnce(() => { throw new Error('cannot attach'); });
  expect(() => createPreviewAudio([
    { element: {} as HTMLAudioElement, kind: 'microphone' },
    { element: {} as HTMLAudioElement, kind: 'system' },
  ], settings, context as unknown as AudioContext)).toThrow('cannot attach');
  expect(context.close).toHaveBeenCalledTimes(1);
  expect(sources[0].disconnect).toHaveBeenCalledTimes(1);
  expect(gains[0].disconnect).toHaveBeenCalledTimes(1);
});

it('uses the latest gate setting when loading finishes, resets on seek, and releases its worklet', async () => {
  const { context, gains } = fixture();
  let loaded!: () => void;
  context.audioWorklet.addModule.mockImplementation(() => new Promise<void>(resolve => { loaded = resolve; }));
  const mic = { addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as HTMLAudioElement;
  const graph = createPreviewAudio([{ element: mic, kind: 'microphone' }], settings, context as unknown as AudioContext);
  graph.update({ ...settings, noiseGate: true });
  expect(gains[0].connect).not.toHaveBeenCalled();
  loaded();
  await graph.setPlaying(true);
  const gate = gains[0].connect.mock.calls[0][0];
  expect(gate.port.postMessage).toHaveBeenCalledWith({ enabled: true });
  const reset = vi.mocked(mic.addEventListener).mock.calls[0][1] as () => void;
  reset();
  expect(gate.port.postMessage).toHaveBeenCalledWith({ reset: true });
  graph.update(settings);
  expect(gate.port.postMessage).toHaveBeenLastCalledWith({ enabled: false });
  graph.dispose();
  expect(mic.removeEventListener).toHaveBeenCalledWith('seeking', reset);
  expect(gate.disconnect).toHaveBeenCalledTimes(1);
  expect(gate.port.close).toHaveBeenCalledTimes(1);
});

it('does not attach a worklet after disposal during loading', async () => {
  const { context, gains } = fixture();
  let loaded!: () => void;
  context.audioWorklet.addModule.mockImplementation(() => new Promise<void>(resolve => { loaded = resolve; }));
  const graph = createPreviewAudio([{ element: {} as HTMLAudioElement, kind: 'microphone' }], settings, context as unknown as AudioContext);
  graph.dispose(); loaded();
  await Promise.resolve();
  expect(gains[0].connect).not.toHaveBeenCalled();
});

it('multiplies independent clip levels by project gains and retains them after settings changes', () => {
  const { context, gains } = fixture();
  const graph = createPreviewAudio([
    { element: {} as HTMLAudioElement, kind: 'microphone' },
    { element: {} as HTMLAudioElement, kind: 'system' },
  ], settings, context as unknown as AudioContext);
  graph.setClipVolumes(0.25, 1);
  expect(gains[0].gain.setTargetAtTime.mock.lastCall[0]).toBe(2.5);
  expect(gains[1].gain.setTargetAtTime.mock.lastCall[0]).toBeCloseTo(0.501187, 5);
  graph.update({ ...settings, micGain: 0, systemMuted: true });
  expect(gains[0].gain.setTargetAtTime.mock.lastCall[0]).toBe(0.25);
  expect(gains[1].gain.setTargetAtTime.mock.lastCall[0]).toBe(0);
  const count = gains[0].gain.setTargetAtTime.mock.calls.length;
  graph.setClipVolumes(0.25, 1);
  expect(gains[0].gain.setTargetAtTime).toHaveBeenCalledTimes(count);
  graph.setClipVolumes(0, 1);
  expect(gains[0].gain.setTargetAtTime.mock.lastCall[0]).toBe(0);
  graph.dispose();
});

it('preview mute silences both channels and restores the latest project and clip gains', () => {
  const { context, gains } = fixture();
  const graph = createPreviewAudio([
    { element: { addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as HTMLAudioElement, kind: 'microphone' },
    { element: {} as HTMLAudioElement, kind: 'system' },
  ], settings, context as unknown as AudioContext);
  graph.setMuted(true);
  expect(gains.map(g => g.gain.setTargetAtTime.mock.lastCall[0])).toEqual([0, 0]);
  graph.update({ ...settings, micGain: 0, systemMuted: true });
  graph.setClipVolumes(0.25, 0.5);
  expect(gains.map(g => g.gain.setTargetAtTime.mock.lastCall[0])).toEqual([0, 0]);
  graph.setMuted(false);
  expect(gains.map(g => g.gain.setTargetAtTime.mock.lastCall[0])).toEqual([0.25, 0]);
  expect(context.createMediaElementSource).toHaveBeenCalledTimes(2);
  graph.dispose();
});
