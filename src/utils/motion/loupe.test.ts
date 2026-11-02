import { expect, it } from 'vitest';
import { projectLoupeToPreview, type LoupeFrame, type VideoTransform } from './loupe';

const frame: LoupeFrame = {center_x:0.75,center_y:0.5,radius:0.125,zoom:2,presence:1,
  bevel:0,chromatic_aberration:0,shadow_alpha:0};
const source = {width:1000,height:500};
const crop = {x:0.5,y:0,width:0.5,height:1};
const transform: VideoTransform = {scale:1,offsetX:0,offsetY:0,viewportWidth:240,viewportHeight:280,
  planeWidth:200,planeHeight:200};
function pixels(projected: ReturnType<typeof projectLoupeToPreview>,view=transform) {
  return {x:projected.x*view.viewportWidth,y:(1-projected.y)*view.viewportHeight,
    radius:projected.radius*view.viewportWidth};
}

it('matches native cropped source projection into a padded stage',()=>{
  const projected = pixels(projectLoupeToPreview(frame,source,crop,transform));
  expect(projected.x).toBeCloseTo(120);
  expect(projected.y).toBeCloseTo(140);
  expect(projected.radius).toBeCloseTo(50);
});

it('matches native fixed-display and moving-window camera projections',()=>{
  // Camera scale 2, focus (.75,.25): display samples x=.5..1, y=0...5.
  const display = {...transform,scale:2,offsetX:-100,offsetY:-100};
  const projectedDisplay = pixels(projectLoupeToPreview(frame,source,crop,display));
  expect(projectedDisplay.x).toBeCloseTo(20);
  expect(projectedDisplay.y).toBeCloseTo(240);
  expect(projectedDisplay.radius).toBeCloseTo(100);
  const window = {...transform,scale:2,offsetX:-50,offsetY:-50};
  const projectedWindow = pixels(projectLoupeToPreview(frame,source,crop,window));
  expect(projectedWindow.x).toBeCloseTo(70);
  expect(projectedWindow.y).toBeCloseTo(190);
  expect(projectedWindow.radius).toBeCloseTo(100);
});

it('keeps off-stage centers outside and converts vertical origin once',()=>{
  const view = {...transform,viewportWidth:100,viewportHeight:100,planeWidth:100,planeHeight:100};
  const topLeft = pixels(projectLoupeToPreview({...frame,center_x:-0.1,center_y:0},
    {width:100,height:100},undefined,view),view);
  expect(topLeft.x).toBeCloseTo(-10);
  expect(topLeft.y).toBeCloseTo(0);
});

it('uses resolved pixel crop edges for odd source dimensions',()=>{
  const oddSource = {width:1919,height:1079};
  const oddCrop = {x:0.25,y:0.1,width:0.5,height:0.8};
  const anchor = {...frame,center_x:480/1919,center_y:108/1079};
  const projected = pixels(projectLoupeToPreview(anchor,oddSource,oddCrop,transform));
  expect(projected.x).toBeCloseTo(20);
  expect(projected.y).toBeCloseTo(40);
  expect(projected.radius).toBeCloseTo(anchor.radius*1919/959*200);
});

it('projects shadow reach through crop and camera while preserving source lighting',()=>{
  const projected=projectLoupeToPreview(frame,source,crop,transform);
  expect(projected.shadowReach*transform.viewportWidth).toBeCloseTo(360);
  expect(projected.sourcePixelScale*transform.viewportWidth).toBeCloseTo(0.4);
  expect(projected.lightDirection[0]).toBeCloseTo(-0.4472136);
  expect(projected.lightDirection[1]).toBeCloseTo(0.8944272);
  expect(projected.lightTilt).toBeCloseTo(0.69877124);
  const zoomed=projectLoupeToPreview(frame,source,crop,{...transform,scale:2});
  expect(zoomed.shadowReach).toBeCloseTo(projected.shadowReach*2);
  expect(zoomed.sourcePixelScale).toBeCloseTo(projected.sourcePixelScale*2);
  expect(zoomed.lightDirection).toEqual(projected.lightDirection);
  expect(zoomed.lightTilt).toEqual(projected.lightTilt);
  const coincident=projectLoupeToPreview({...frame,center_x:0.5,center_y:-0.5},source,crop,transform);
  expect(coincident.lightDirection).toEqual([0,1]);
  expect(coincident.lightTilt).toBe(0);
});
