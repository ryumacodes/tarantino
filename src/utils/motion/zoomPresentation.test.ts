import { expect, it } from 'vitest';
import type { ZoomBlock } from '../../stores/editor/types';
import { zoomPresentationUpdate } from './zoomPresentation';
import { exportZoomBlock } from './zoomTrajectory';

const lens = {radius:0.6,glass:true,bevel:0.4,chromatic_aberration:0.7,shadow_alpha:0.2};
const block: ZoomBlock = {id:'zoom',start_time:0,end_time:1000,zoom_factor:3,
  click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,is_manual:true,loupe:lens};

it('retains lens settings through repeated mode changes without rendering the inactive lens', () => {
  const screen = {...block,...zoomPresentationUpdate(block,'screen')};
  expect(screen.inactive_loupe).toEqual(lens);
  expect(screen.inactive_loupe).not.toBe(lens);
  expect(exportZoomBlock(screen).loupe).toBeNull();
  expect(exportZoomBlock(screen)).not.toHaveProperty('inactive_loupe');
  const screenAgain = {...screen,...zoomPresentationUpdate(screen,'screen')};
  const magnifier = {...screenAgain,...zoomPresentationUpdate(screenAgain,'magnifier')};
  expect(magnifier.loupe).toEqual(lens);
  expect(magnifier.inactive_loupe).toBeUndefined();
  expect(exportZoomBlock(magnifier).loupe).toEqual(lens);
});

it('prefers edited active settings over dormant settings', () => {
  const changed = {...block,inactive_loupe:lens,loupe:{...lens,radius:0.3}};
  const screen = {...changed,...zoomPresentationUpdate(changed,'screen')};
  expect(zoomPresentationUpdate(screen,'magnifier').loupe?.radius).toBe(0.3);
});
