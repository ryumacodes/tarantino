export interface PreviewMedia {
  currentTime: number; playbackRate: number; preservesPitch: boolean;
  paused: boolean; seeking: boolean; readyState: number; muted: boolean;
  play(): Promise<void>; pause(): void;
}
/** A single controller per element avoids repeated play requests during async starts. */
export function createMediaSynchronizer(media: PreviewMedia, reportError: (error: unknown) => void) {
  let previousClip: string | null | undefined;
  let previousRevision: number | undefined;
  let pendingPlay = false;
  let wantsPlay = false;
  let failed = false;
  const unsupportedRates = new Set<number>();
  return (sample: { sourceTime: number | null; rate: number; clipId: string | null; muted: boolean }, playing: boolean, audio = false, revision?: number) => {
    const gap = sample.sourceTime === null;
    wantsPlay = playing && !gap;
    media.muted = audio ? sample.muted || gap : true;
    media.preservesPitch = true;
    // Some WebViews reject fast rates. Keep the edited clock and frame seeking
    // alive instead of throwing out of the preview loop or playing at the wrong speed.
    if (!unsupportedRates.has(sample.rate) && media.playbackRate !== sample.rate) {
      try { media.playbackRate = sample.rate; }
      catch (error) { unsupportedRates.add(sample.rate); reportError(error); }
    }
    const seekOnly = unsupportedRates.has(sample.rate);
    wantsPlay = wantsPlay && !seekOnly;
    if (seekOnly) media.muted = true;
    if (gap) { media.pause(); previousClip = undefined; previousRevision = undefined; failed = false; return; }
    const target = sample.sourceTime! / 1000;
    const changedClip = previousClip !== sample.clipId;
    const explicitSeek = revision !== undefined && revision !== previousRevision;
    const tolerance = playing ? 0.08 : 0.001;
    if (media.readyState >= 1 && !media.seeking) {
      if (changedClip || explicitSeek || Math.abs(media.currentTime - target) > tolerance) media.currentTime = target;
      previousClip = sample.clipId;
      previousRevision = revision;
    }
    if (!playing || seekOnly) { media.pause(); failed = false; }
    else if (media.paused && !pendingPlay && (!failed || changedClip) && media.readyState >= 2) {
      pendingPlay = true;
      media.play().then(() => {
        // A pause or gap may arrive while the browser resolves play().
        if (!wantsPlay) media.pause();
      }).catch(error => { failed = true; reportError(error); })
        .finally(() => { pendingPlay = false; });
    }
  };
}
