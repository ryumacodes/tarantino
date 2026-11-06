import {Matrix3, type Texture, type WebGLRenderer} from 'three';
import {LoupePyramid} from './LoupePyramid';

/** Keep decoded pixels filtered independently of the animated camera crop. */
export class SourceTextureFilter {
  private pyramid: LoupePyramid | null = null;
  private last: {source:Texture;version:number;revision:string;transform:Matrix3} | null = null;

  update(renderer:WebGLRenderer,source:Texture,width:number,height:number,transform:Matrix3,revision:string) {
    if(this.pyramid?.width!==width||this.pyramid.height!==height) {
      this.pyramid?.dispose();this.pyramid=new LoupePyramid(width,height);this.last=null;
    }
    const last=this.last;
    if(!last||last.source!==source||last.version!==source.version||last.revision!==revision
      ||!last.transform.elements.every((value,index)=>Math.abs(value-transform.elements[index])<1e-10)) {
      this.pyramid.update(renderer,source,transform);
      this.last={source,version:source.version,revision,transform:transform.clone()};
    }
    return this.pyramid.target.texture;
  }

  dispose() {this.pyramid?.dispose();this.pyramid=null;this.last=null;}
}
