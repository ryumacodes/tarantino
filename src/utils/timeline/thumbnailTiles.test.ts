import { expect, it } from 'vitest';
import { thumbnailTiles } from './thumbnailTiles';
import type { TimelineClip, TimelineTrack } from '../../stores/editor/types';
const clip = (id:string, startTime:number, sourceIn:number, sourceOut:number, playbackRate=1):TimelineClip =>
  ({id,name:id,type:'video',trackId:'v',startTime,sourceIn,sourceOut,playbackRate,duration:(sourceOut-sourceIn)/playbackRate,enabled:true,locked:false});
const track:TimelineTrack = {id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
it('maps reordered and speed-adjusted clips through source time, not edited duration', () => {
  const clips=[clip('later',0,6000,10000,2),clip('earlier',2000,0,1000)];
  const tiles=thumbnailTiles({clips,tracks:[track],duration:10000},'/take',11,0.1);
  expect(tiles.map(tile=>tile.sourceTime)).toEqual([7000,9000,500]);
  expect(tiles.map(tile=>tile.index)).toEqual([7,9,0]);
  expect(tiles.map(tile=>[tile.left,tile.width])).toEqual([[0,100],[100,100],[200,100]]);
});
it('clips tiles at cuts and leaves gaps and foreign sources without misleading frames', () => {
  const clips=[clip('a',0,0,1500),{...clip('foreign',2000,0,1000),sourceFilePath:'/other'}];
  const tiles=thumbnailTiles({clips,tracks:[track],duration:10000},'/take',11,0.1);
  expect(tiles.map(tile=>[tile.left,tile.width])).toEqual([[0,100],[100,50]]);
});
it('uses the same visibility and layer ordering as the preview', () => {
  const clips=[clip('back',0,0,2000),{...clip('front',500,5000,6000),trackId:'front'}];
  const front={...track,id:'front',order:-1};
  const tiles=thumbnailTiles({clips,tracks:[track,front],duration:10000},'/take',11,0.1);
  expect(tiles.map(tile=>tile.sourceTime)).toEqual([250,5500,1750]);
  expect(thumbnailTiles({clips,tracks:[{...track,visible:false},{...front,visible:false}],duration:10000},'/take',11,0.1)).toEqual([]);
});
it('handles legacy recordings, empty caches and invalid scales', () => {
  const state={clips:[],tracks:[],duration:1000};
  expect(thumbnailTiles(state,'/take',1,0.1)).toHaveLength(1);
  expect(thumbnailTiles(state,'/take',0,0.1)).toEqual([]);
  expect(thumbnailTiles(state,'/take',1,Infinity)).toEqual([]);
});

it('prefers retained source frames over a closer frame in deleted footage', () => {
  const clips=[clip('a',0,0,1000),clip('c',1000,2000,3000)];
  const tiles=thumbnailTiles({clips,tracks:[track],duration:3000},'/take',4,0.1,30);
  expect(tiles.filter(tile=>tile.left<100).every(tile=>tile.index===0)).toBe(true);
  expect(tiles.filter(tile=>tile.left>=100).every(tile=>tile.index>=2)).toBe(true);
});
it('requests a source sample when a short cut has no cached frame', () => {
  const tiles=thumbnailTiles({clips:[clip('short',0,1200,1300)],tracks:[track],duration:10000},'/take',11,0.1);
  expect(tiles).toHaveLength(1);
  expect(tiles[0]).toMatchObject({needsSourceSample:true,sourceTime:1250});
});
it('bounds thumbnail work to the viewport and keeps tile identity stable while scrolling', () => {
  const state={clips:[clip('long',0,0,3600000)],tracks:[track],duration:3600000};
  const full=thumbnailTiles(state,'/take',50,0.1);
  const viewport=thumbnailTiles(state,'/take',50,0.1,100,{left:100050,right:101050});
  expect(full).toHaveLength(3600);
  expect(viewport).toHaveLength(11);
  expect(viewport).toEqual(full.filter(tile=>tile.left+tile.width>100050 && tile.left<101050));
  const next=thumbnailTiles(state,'/take',50,0.1,100,{left:100100,right:101100});
  expect(next.slice(0,10)).toEqual(viewport.slice(1));
});
it('skips offscreen short cuts before scheduling source decoding', () => {
  const state={clips:Array.from({length:1000},(_,i)=>clip(String(i),i*100,1000+i*200,1100+i*200)),tracks:[track],duration:300000};
  const tiles=thumbnailTiles(state,'/take',5,1,100,{left:5000,right:6000});
  expect(tiles).toHaveLength(10);
  expect(tiles.every(tile=>tile.left>=5000 && tile.left<6000 && tile.needsSourceSample)).toBe(true);
  expect(thumbnailTiles(state,'/take',5,1,100,{left:10,right:10})).toEqual([]);
});
