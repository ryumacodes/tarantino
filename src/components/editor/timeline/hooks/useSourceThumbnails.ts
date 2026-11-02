import { useEffect, useMemo, useReducer } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { SourceThumbnailCache, type SourceThumbnail } from '../../../../utils/timeline/sourceThumbnailCache';
import type { ThumbnailTile } from '../../../../utils/timeline/thumbnailTiles';

export function useSourceThumbnails(path: string | null, tiles: ThumbnailTile[]) {
  const [,refresh] = useReducer(count=>count+1,0);
  const cache = useMemo(()=>new SourceThumbnailCache(times=>invoke<SourceThumbnail[]>(
    'extract_timeline_thumbnail_samples',{videoPath:path,sourceTimesMs:times,thumbnailWidth:160})),[path]);
  useEffect(()=>{
    if (!path) return;
    let cancelled=false;
    const timer=setTimeout(async()=>{
      const missing=tiles.filter(tile=>tile.needsSourceSample).map(tile=>tile.sourceTime);
      for (let start=0;start<missing.length;start+=16) {
        if (cancelled) return;
        await cache.ensure(missing.slice(start,start+16));
        if (!cancelled) refresh();
      }
    },200);
    return ()=>{cancelled=true;clearTimeout(timer);};
  },[path,tiles,cache]);
  return (tile: ThumbnailTile)=>cache.get(tile.sourceTime);
}
