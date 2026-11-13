import {expect,it} from 'vitest';
import {Texture} from 'three';
import {sourceFrameTime} from './SourceTextureFilter';

it('ignores clock drift only when video texture versions track decoded frames',()=>{
  const image={currentTime:1,requestVideoFrameCallback:()=>0};
  const texture=Object.assign(new Texture(image),{isVideoTexture:true});
  expect(sourceFrameTime(texture)).toBe(0);
  image.currentTime=1.005;
  expect(sourceFrameTime(texture)).toBe(0);
  texture.needsUpdate=true;
  expect(texture.version).toBe(1);
  expect(sourceFrameTime(texture)).toBe(0);
  expect(sourceFrameTime(new Texture(image))).toBe(1.005);
});

it('retains the playback clock without decoded-frame notifications',()=>{
  const image={currentTime:2};
  const texture=Object.assign(new Texture(image),{isVideoTexture:true});
  expect(sourceFrameTime(texture)).toBe(2);
  image.currentTime=2.1;
  expect(sourceFrameTime(texture)).toBe(2.1);
  expect(sourceFrameTime(new Texture())).toBe(0);
});
