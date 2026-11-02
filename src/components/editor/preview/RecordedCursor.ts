import {DataTexture,NoColorSpace,LinearFilter,type WebGLRenderer} from 'three';
import {LoupePyramid} from './LoupePyramid';
import {cursorTextureBytes,recordedCursorIndex,type RecordedCursorData} from '../../../utils/motion/recordedCursor';

export class RecordedCursor {
  private readonly textures=new Map<number,LoupePyramid>();
  private selected:number|null=null;
  constructor(private readonly data:RecordedCursorData) {}

  select(time:number|null,enabled:boolean) {
    this.selected=enabled?recordedCursorIndex(this.data,time):null;
  }
  prepare(renderer:WebGLRenderer) {
    const id=this.selected;
    if(id===null)return null;
    const image=this.data.images[id]!;
    let pyramid=this.textures.get(id);
    if(!pyramid) {
      const texture=new DataTexture(cursorTextureBytes(image),image.width,image.height);
      texture.colorSpace=NoColorSpace;texture.minFilter=LinearFilter;texture.magFilter=LinearFilter;
      texture.generateMipmaps=false;texture.needsUpdate=true;
      pyramid=new LoupePyramid(image.width,image.height,true);
      try {pyramid.update(renderer,texture);} catch(error) {pyramid.dispose();throw error;}
      finally {texture.dispose();}
      this.textures.set(id,pyramid);
    }
    return {texture:pyramid.target.texture,rect:image.rect,
      size:[image.width,image.height,pyramid.levels.length]};
  }
  dispose() {for(const texture of this.textures.values())texture.dispose();this.textures.clear();}
}
