import gateModuleUrl from './noiseGate.worklet?worker&url';
import type { AudioSettings } from '../../stores/editor/types';

export interface PreviewAudioTrack {
  element: HTMLAudioElement;
  kind: 'microphone' | 'system';
}

export function channelGain(settings: AudioSettings, kind: PreviewAudioTrack['kind']): number {
  const muted = kind === 'microphone' ? settings.micMuted : settings.systemMuted;
  const db = kind === 'microphone' ? settings.micGain : settings.systemGain;
  return muted ? 0 : 10 ** ((Number.isFinite(db) ? db : 0) / 20);
}

/** One owned audio graph per set of media elements; never attach a source twice. */
export function createPreviewAudio(
  tracks: readonly PreviewAudioTrack[],
  settings: AudioSettings,
  context: AudioContext = new AudioContext(),
) {
  let disposed = false;
  let currentSettings = settings;
  let previewMuted = false;
  let clipMicVolume = 1;
  let clipSystemVolume = 1;
  const level = (kind: PreviewAudioTrack['kind']) => (previewMuted ? 0 : channelGain(currentSettings, kind))
    * (kind === 'microphone' ? clipMicVolume : clipSystemVolume);
  const routes: { track: PreviewAudioTrack; source: MediaElementAudioSourceNode; gain: GainNode; gate?: AudioWorkletNode; reset?: () => void }[] = [];
  try {
    for (const track of tracks) {
      const source = context.createMediaElementSource(track.element);
      const gain = context.createGain();
      routes.push({ track, source, gain });
      track.element.volume = 1;
      track.element.muted = false;
      gain.gain.setValueAtTime(channelGain(settings, track.kind), context.currentTime);
      source.connect(gain);
      if (track.kind === 'system') gain.connect(context.destination);
    }
  } catch (error) {
    for (const route of routes) { route.source.disconnect(); route.gain.disconnect(); }
    void context.close().catch(() => {});
    throw error;
  }
  const loadGate = () => {
    try { return context.audioWorklet.addModule(gateModuleUrl); }
    catch (error) { return Promise.reject(error); }
  };
  const ready = tracks.some(track => track.kind === 'microphone')
    ? loadGate().then(() => {
      if (disposed) return;
      for (const route of routes) {
        if (route.track.kind !== 'microphone') continue;
        const gate = new AudioWorkletNode(context, 'microphone-gate');
        route.gate = gate;
        gate.port.postMessage({ enabled: currentSettings.noiseGate });
        route.reset = () => gate.port.postMessage({ reset: true });
        route.track.element.addEventListener('seeking', route.reset);
        route.gain.connect(gate);
        gate.connect(context.destination);
      }
    }) : Promise.resolve();
  // Attach a rejection handler immediately, even if playback has not started yet.
  void ready.catch(error => {
    for (const { track } of routes) track.element.muted = true;
    console.error('Could not load microphone processing:', error);
  });
  return {
    setMuted(muted: boolean) {
      if (disposed || previewMuted === muted) return;
      previewMuted = muted;
      for (const { track, gain } of routes) {
        gain.gain.setTargetAtTime(level(track.kind), context.currentTime, 0.008);
      }
    },
    update(next: AudioSettings) {
      if (disposed) return;
      currentSettings = next;
      for (const { track, gain, gate } of routes) {
        gate?.port.postMessage({ enabled: next.noiseGate });
        // A short smoothing interval avoids clicks without changing the final gain.
        gain.gain.setTargetAtTime(level(track.kind), context.currentTime, 0.008);
      }
    },
    setClipVolumes(mic: number, system: number) {
      if (disposed || (mic === clipMicVolume && system === clipSystemVolume)) return;
      clipMicVolume = mic;
      clipSystemVolume = system;
      for (const { track, gain } of routes) {
        gain.gain.setTargetAtTime(level(track.kind), context.currentTime, 0.008);
      }
    },
    async setPlaying(playing: boolean) {
      if (disposed || context.state === 'closed') return;
      await ready;
      if (disposed) return;
      if (playing) await context.resume();
      else await context.suspend();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const { source, gain, gate, reset, track } of routes) {
        if (reset) track.element.removeEventListener('seeking', reset);
        source.disconnect(); gain.disconnect(); gate?.disconnect(); gate?.port.close();
      }
      void context.close().catch(() => {});
    },
  };
}
