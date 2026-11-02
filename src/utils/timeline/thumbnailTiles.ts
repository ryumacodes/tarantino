import { visibleTimelineSegments, type TimelineState } from './previewTimeline';

export interface ThumbnailTile { key: string; left: number; width: number; index: number; sourceTime: number; needsSourceSample: boolean; }

/** Reuse the recording's thumbnail cache on the edited clock, without decoding on edits. */
export function thumbnailTiles(state: TimelineState, sourcePath: string | null, count: number,
  pixelsPerMs: number, tileWidth = 100, viewport?: { left: number; right: number }): ThumbnailTile[] {
  if (count <= 0 || state.duration <= 0 || !Number.isFinite(pixelsPerMs) || pixelsPerMs <= 0
    || !Number.isFinite(tileWidth) || tileWidth <= 0) return [];
  if (viewport && (!Number.isFinite(viewport.left) || !Number.isFinite(viewport.right)
    || viewport.right <= viewport.left)) return [];
  const segments = visibleTimelineSegments(state);
  const ranges = !state.tracks.some(track => track.type === 'video' || track.type === 'screen')
    ? [{start:0,end:state.duration,clip:null}]
    : segments.map(segment => ({start:segment.start,end:segment.end,clip:segment.entry.clip}));
  return ranges.flatMap(({start,end,clip}) => {
    if (clip?.sourceFilePath && clip.sourceFilePath !== sourcePath) return [];
    const left = Math.max(0,start)*pixelsPerMs;
    const right = end*pixelsPerMs;
    if (right <= left || (viewport && (right <= viewport.left || left >= viewport.right))) return [];
    // Anchor the grid to the clip, not the viewport, so scrolling reuses cached samples.
    const firstTile = viewport ? Math.max(0, Math.floor((viewport.left-left)/tileWidth)) : 0;
    const visibleRight = viewport ? Math.min(right,viewport.right) : right;
    let first = count, last = -1;
    if (clip && count > 1) {
        const interval = state.duration/(count-1);
        const safeEnd = Math.max(0,state.duration-200);
        const retained = Array.from({length:count},(_,i) => i).filter(i => {
          const sampledTime = Math.min(i*interval,safeEnd);
          return sampledTime >= clip.sourceIn && sampledTime < clip.sourceOut;
        });
        first = retained[0] ?? count;
        last = retained[retained.length-1] ?? -1;
    }
    const tiles: ThumbnailTile[] = [];
    for (let x=left+firstTile*tileWidth; x<visibleRight; x+=tileWidth) {
      const width = Math.min(tileWidth,right-x);
      const time = (x+width/2)/pixelsPerMs;
      const sourceTime = clip ? clip.sourceIn+(time-clip.startTime)*clip.playbackRate : time;
      // Native cache samples are evenly spaced from recording start to end;
      // the final sample is clamped to the last decodable frame.
      let index = Math.max(0,Math.min(count-1,Math.round(sourceTime/state.duration*(count-1))));
      if (first <= last) index = Math.max(first,Math.min(last,index));
      tiles.push({key:`${clip?.id ?? 'source'}:${start}:${x}`,left:x,width,index,sourceTime,needsSourceSample:!!clip && first>last});
    }
    return tiles;
  });
}
