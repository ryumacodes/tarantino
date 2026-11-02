import {sampleWebcam} from "../camera/webcamTimeline";
import type {VisualSettings,WebcamKeyframe} from "../../stores/editor/types";
import { previewSample, timelineDuration, type TimelineState } from '../timeline/previewTimeline';
import { shortcutAt, type ShortcutInterval } from './intervals';

export interface ShortcutSource {
  intervals: readonly ShortcutInterval[];
  hidden?: ReadonlySet<number>;
}

/** Sample output time in milliseconds using the video's canonical edit mapping.
 * Resolve data per source so labels cannot leak from a different recording.
 * The export range is half-open; the editor can separately hold its final frame.
 */
export function shortcutAtOutputTime(
  state: TimelineState & { trimStart: number; trimEnd: number; hasWebcam?:boolean; visualSettings?:VisualSettings; webcamKeyframes?:WebcamKeyframe[] },
  outputTime: number,
  source: (path: string | undefined) => ShortcutSource | undefined,
): ShortcutInterval | null {
  const end = Math.min(state.trimEnd, timelineDuration(state));
  const time = state.trimStart + outputTime;
  if (!Number.isFinite(outputTime) || outputTime < 0 || !Number.isFinite(time)
    || time >= end) return null;
  const sample = previewSample(state, time);
  if (sample.sourceTime === null) return null;
  if(state.hasWebcam&&state.visualSettings) {
    const camera=sampleWebcam(state.visualSettings,state.webcamKeyframes??[],sample.sourceTime);
    if(camera.visible&&camera.layout==='fullscreen')return null;
  }
  const data = source(sample.sourceFilePath);
  return data ? shortcutAt(data.intervals, sample.sourceTime, data.hidden) : null;
}
