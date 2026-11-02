/** Linked-channel RMS downward expansion, with the same settings as native export. */
export class NoiseGate {
  private envelope = 0;
  private readonly attack: number;
  private readonly release: number;
  private readonly threshold = Math.log(0.015 ** 2);
  private readonly halfKnee = Math.log(2.828427125) / 2;
  constructor(sampleRate: number) {
    this.attack = Math.min(1, 4000 / (5 * sampleRate));
    this.release = Math.min(1, 4000 / (80 * sampleRate));
  }
  reset() { this.envelope = 0; }
  process(input: readonly Float32Array[], output: readonly Float32Array[], enabled: boolean) {
    const frames = output[0]?.length ?? 0;
    for (let frame = 0; frame < frames; frame++) {
      let level = 0;
      for (const channel of input) level += Math.abs(channel[frame] ?? 0);
      level = (level / Math.max(1, input.length)) ** 2;
      this.envelope += (level - this.envelope) * (level > this.envelope ? this.attack : this.release);
      let gain = 1;
      if (enabled && this.envelope > 0) {
        const distance = Math.log(this.envelope) - this.threshold;
        if (distance < this.halfKnee) {
          // A quadratic soft knee joins the expanded and unity slopes continuously.
          const reduction = distance <= -this.halfKnee
            ? 7 * distance
            : -7 * (distance - this.halfKnee) ** 2 / (4 * this.halfKnee);
          gain = Math.max(0.06125, Math.exp(reduction));
        }
      }
      for (let channel = 0; channel < output.length; channel++) {
        output[channel][frame] = (input[channel]?.[frame] ?? 0) * gain;
      }
    }
  }
}
