import { NoiseGate } from './noiseGate';
declare const sampleRate: number;
declare class AudioWorkletProcessor { readonly port: MessagePort; }
declare function registerProcessor(name: string, processor: typeof AudioWorkletProcessor): void;

class MicrophoneGate extends AudioWorkletProcessor {
  private readonly gate = new NoiseGate(sampleRate);
  private enabled = false;
  constructor() {
    super();
    this.port.onmessage = ({ data }) => {
      if (data.reset) this.gate.reset();
      if (typeof data.enabled === 'boolean') this.enabled = data.enabled;
    };
  }
  process(inputs: Float32Array[][], outputs: Float32Array[][]) {
    this.gate.process(inputs[0] ?? [], outputs[0] ?? [], this.enabled);
    return true;
  }
}
registerProcessor('microphone-gate', MicrophoneGate);
