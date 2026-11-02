export interface PreviewMotionSample {
  playing: boolean;
  revision: number;
  clipId: string | null;
  sourceTime: number | null;
  sourcePath: string | null;
  rate: number;
  trajectory: object;
  clips: object;
  tracks: object;
}

/** Explicit seeks and edit revisions must not borrow motion from a previous view. */
export function continuousPreviewMotion(previous:PreviewMotionSample|null,current:PreviewMotionSample) {
  return !!previous && previous.playing && current.playing
    && previous.sourceTime!==null && current.sourceTime!==null
    && current.sourceTime>=previous.sourceTime
    && previous.revision===current.revision && previous.clipId===current.clipId
    && previous.sourcePath===current.sourcePath && previous.rate===current.rate
    && previous.trajectory===current.trajectory
    && previous.clips===current.clips && previous.tracks===current.tracks;
}
