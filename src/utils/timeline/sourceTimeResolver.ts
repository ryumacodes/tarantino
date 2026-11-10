import { visibleTimelineSegments, type TimelineState } from './previewTimeline';

export interface SourceTimeResolver {
  toPlayback: (source: number) => number;
  toSource: (playback: number) => number;
}

/** Resolve within the selected occurrence, stopping at source repetitions or reordering. */
export function sourceTimeResolver(state: TimelineState, playbackTime: number): SourceTimeResolver | null {
  const segments = visibleTimelineSegments(state).map(({start,end,entry:{clip}}) => ({
    start,end,sourceStart:clip.sourceIn+(start-clip.startTime)*clip.playbackRate,
    sourceEnd:clip.sourceIn+(end-clip.startTime)*clip.playbackRate,
    rate:clip.playbackRate,path:clip.sourceFilePath,
  }));
  const anchor = segments.findIndex(segment => segment.start <= playbackTime && playbackTime < segment.end);
  if (anchor < 0) return null;
  const connected = (left: typeof segments[number], right: typeof segments[number]) =>
    Math.abs(left.end-right.start)<1e-7 && left.sourceEnd <= right.sourceStart+1e-7 && left.path === right.path;
  let first=anchor, last=anchor;
  while (first>0 && connected(segments[first-1],segments[first])) first--;
  while (last+1<segments.length && connected(segments[last],segments[last+1])) last++;
  const links=segments.slice(first,last+1);
  return {
    toPlayback(source) {
      let link=links[0];
      for (const candidate of links) {
        if (candidate.sourceStart>source) break;
        link=candidate;
      }
      return link.start+(Math.max(link.sourceStart,Math.min(link.sourceEnd,source))-link.sourceStart)/link.rate;
    },
    toSource(playback) {
      const link=links.find(candidate => playback<=candidate.end) ?? links[links.length-1];
      return link.sourceStart+(Math.max(link.start,Math.min(link.end,playback))-link.start)*link.rate;
    },
  };
}
