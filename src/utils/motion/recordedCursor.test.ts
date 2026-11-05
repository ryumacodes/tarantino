import {describe,it,expect} from 'vitest';
import {cursorTextureBytes,recordedCursorIndex,type RecordedCursorData} from './recordedCursor';

describe('recorded cursor textures',()=>{
  const image={width:1,height:2,rgba_base64:btoa(String.fromCharCode(255,0,0,255,0,255,0,128)),rect:[0,0,1,2] as [number,number,number,number]};
  it('flips rows once without changing channels or alpha',()=>{
    expect([...cursorTextureBytes(image)]).toEqual([0,255,0,128,255,0,0,255]);
    expect(()=>cursorTextureBytes({...image,height:3})).toThrow('Invalid cursor bitmap');
  });
  it('uses exact source-time boundaries and falls back for unavailable images',()=>{
    const data:RecordedCursorData={images:[image,image,null],changes:[
      {timestamp:50,image_id:0},{timestamp:500,image_id:1},{timestamp:700,image_id:2}]};
    expect([null,NaN,-1,0,49.99,50,499.99,500,699,700,1000].map(t=>recordedCursorIndex(data,t)))
      .toEqual([null,null,null,null,null,0,0,1,1,null,null]);
    // Seeking back uses the correct earlier image, independent of render history.
    expect(recordedCursorIndex(data,50)).toBe(0);
  });
  it('preserves promoted appearance order and the known arrow fallback',()=>{
    const data:RecordedCursorData={images:[image,image,image],fallback_image_id:0,changes:[
      {timestamp:100,image_id:1},{timestamp:300,image_id:1},
      {timestamp:551,image_id:0},{timestamp:400,image_id:2}]};
    expect([0,100,300,400,550,551,551.01,600].map(t=>recordedCursorIndex(data,t)))
      .toEqual([0,1,1,1,1,0,2,2]);
    expect(recordedCursorIndex({...data,changes:[{timestamp:100,image_id:99}]},100)).toBe(0);
    expect(recordedCursorIndex({...data,changes:[{timestamp:100,image_id:1},{timestamp:100,image_id:2}]},100)).toBe(1);
  });
  it('returns to procedural shapes when a preset only supplies a hand',()=>{
    const data:RecordedCursorData={images:[null,image,null],fallback_image_id:0,changes:[
      {timestamp:0,image_id:0},{timestamp:100,image_id:1},{timestamp:200,image_id:2}]};
    expect([0,99.9,100,199.9,200,100,0].map(t=>recordedCursorIndex(data,t)))
      .toEqual([null,null,1,1,null,1,null]);
    expect(recordedCursorIndex({images:[],changes:[]},100)).toBeNull();
  });
});
