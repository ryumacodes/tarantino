import { zoomPresentationUpdate } from './zoomPresentation';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { ZoomBlock } from '../../stores/editor/types';
import { readZoomStyle, rememberEditedZoomStyle } from './zoomStylePreference';

const block: ZoomBlock = { id:'zoom', start_time:1000, end_time:4000, zoom_factor:2,
  click_x:0.5, click_y:0.5, center_x:0.5, center_y:0.5, is_manual:true };
let saved: string | null;
beforeEach(() => {
  saved = null;
  vi.stubGlobal('localStorage', { getItem: () => saved,
    setItem: (_: string, value: string) => { saved = value; } });
});
afterEach(() => vi.unstubAllGlobals());

it('remembers edited presentation while excluding timing, location and speed overrides', () => {
  const edited = { ...block, zoom_factor:3, instant_animation:true, zoom_in_speed:'rapid' as const,
    start_time:2000, center_x:0.8,
    loupe:{radius:0.2, glass:true, bevel:0.3, chromatic_aberration:0.4, shadow_alpha:0.35} };
  rememberEditedZoomStyle(block, edited);
  expect(readZoomStyle()).toEqual({zoom_factor:3, instant_animation:true, loupe:edited.loupe});
  const next = readZoomStyle();
  next.loupe!.radius = 0.9;
  expect(readZoomStyle().loupe?.radius).toBe(0.2);
});

it('does not replace the preference on a timing, target, enable or speed edit', () => {
  rememberEditedZoomStyle(block, {...block,zoom_factor:3});
  rememberEditedZoomStyle(block, {...block,start_time:2000,center_x:0.8,is_disabled:true,zoom_out_speed:'slow'});
  expect(readZoomStyle().zoom_factor).toBe(3);
  rememberEditedZoomStyle(block,{...block,loupe:undefined,instant_animation:false});
  expect(readZoomStyle().zoom_factor).toBe(3);
});

it('remembers returning from magnifier to screen zoom', () => {
  const lens = {...block,loupe:{radius:0.2,glass:true,bevel:0.3,chromatic_aberration:0.4,shadow_alpha:0.35}};
  rememberEditedZoomStyle(lens,block);
  expect(readZoomStyle()).toEqual({zoom_factor:2,instant_animation:false,loupe:null});
});

it('ignores malformed stored styles and tolerates unavailable preference storage', () => {
  for (const value of ['invalid','null','{}','{"zoom_factor":3,"instant_animation":true,"loupe":{}}']) {
    saved=value;
    expect(readZoomStyle().zoom_factor).toBe(2);
  }
  vi.stubGlobal('localStorage',{getItem:()=>{throw Error('unavailable');},setItem:()=>{throw Error('unavailable');}});
  expect(() => rememberEditedZoomStyle(block,{...block,zoom_factor:3})).not.toThrow();
  expect(readZoomStyle().zoom_factor).toBe(2);
});


it('carries dormant lens settings into new screen zooms', () => {
  const magnifier = {...block,loupe:{radius:0.6,glass:true,bevel:0.4,chromatic_aberration:0.7,shadow_alpha:0.2}};
  const screen = {...magnifier,...zoomPresentationUpdate(magnifier,'screen')};
  rememberEditedZoomStyle(magnifier,screen);
  const added = {...block,...readZoomStyle()};
  expect(added.loupe).toBeNull();
  expect(zoomPresentationUpdate(added,'magnifier').loupe).toEqual(magnifier.loupe);
});
