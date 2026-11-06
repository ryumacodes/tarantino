import type {RecordingMask} from '../../../../utils/masks/document';
import {useEffect,useRef,type MutableRefObject} from 'react';
import {useFrame} from '@react-three/fiber';
import type {MeshBasicMaterial,Texture} from 'three';
import {useEditorStore} from '../../../../stores/editor';
import {previewSample} from '../../../../utils/timeline/previewTimeline';
import {resolveSourceCrop,type SourceCrop} from '../../../../utils/sourceCrop';
import {maskPixelBounds} from '../../../../utils/masks/geometry';
import {MaskSource} from '../MaskSource';
import {MaskStage} from '../MaskStage';
import {SourceTextureFilter} from '../SourceTextureFilter';
import {sourceTextureTransform} from '../../../../utils/motion/sourceTextureTransform';

export function useMaskPreview(texture:Texture|null,viewRef?:MutableRefObject<SourceCrop>){
 const material=useRef<MeshBasicMaterial>(null);
 const encodedMask=useRef({value:false});
 const filter=useRef(new SourceTextureFilter());
 const maskRevision=useRef(0);
 const resources=useRef<{key:string;source:MaskSource;stage:MaskStage;last?:{texture:Texture;version:number;time:number;mask:RecordingMask}}|null>(null);
 useEffect(()=>()=>{resources.current?.source.dispose();resources.current?.stage.dispose();resources.current=null;filter.current.dispose();},[]);
 useFrame(({gl})=>{
  if(!material.current||!texture)return;
  if(!material.current.userData.maskDecode){
   material.current.userData.maskDecode=true;
   material.current.onBeforeCompile=shader=>{
    shader.uniforms.maskEncoded=encodedMask.current;
    shader.fragmentShader='uniform bool maskEncoded;\n'+shader.fragmentShader;
    shader.fragmentShader=shader.fragmentShader.replace('#include <map_fragment>',`#include <map_fragment>
     if(maskEncoded)diffuseColor.rgb=mix(pow((diffuseColor.rgb+0.055)/1.055,vec3(2.4)),
       diffuseColor.rgb/12.92,lessThanEqual(diffuseColor.rgb,vec3(0.04045)));`);
   };
   material.current.needsUpdate=true;
  }
  encodedMask.current.value=false;
  const state=useEditorStore.getState(),sample=previewSample(state,state.currentTime);
  let output:Texture|null=sample.sourceTime===null?null:texture;
  const path=sample.sourceFilePath??state.videoFilePath;
  const mask=sample.sourceTime===null?undefined:state.masks.find(item=>!item.disabled&&item.sourceFilePath===path
   &&item.sourceStartMs<=sample.sourceTime!&&item.sourceEndMs>sample.sourceTime!);
  const image=texture.image as {videoWidth?:number;videoHeight?:number;width?:number;height?:number;currentTime?:number};
  const width=image?.videoWidth??image?.width,height=image?.videoHeight??image?.height;
  if(mask&&width&&height){
   const sourceWidth=path===state.videoFilePath?(state.videoWidth??width):(mask.coordinateSpace?.sourceWidth??width);
   const sourceHeight=path===state.videoFilePath?(state.videoHeight??height):(mask.coordinateSpace?.sourceHeight??height);
   // Native export resolves an explicit crop against the primary recording.
   const crop=state.visualSettings.sourceCrop&&state.videoWidth&&state.videoHeight
    ?resolveSourceCrop(state.visualSettings.sourceCrop,state.videoWidth,state.videoHeight)
    :resolveSourceCrop({x:0,y:0,width:1,height:1},sourceWidth,sourceHeight);
   const decodedWidth=Math.max(1,Math.round(crop.width*width/sourceWidth)),decodedHeight=Math.max(1,Math.round(crop.height*height/sourceHeight));
   const scaleX=decodedWidth/crop.width,scaleY=decodedHeight/crop.height;
   const key=[crop.x,crop.y,crop.width,crop.height,sourceWidth,sourceHeight,decodedWidth,decodedHeight].join(':');
   if(resources.current?.key!==key){
    resources.current?.source.dispose();resources.current?.stage.dispose();
    resources.current={key,source:new MaskSource({...crop,sourceWidth,sourceHeight},decodedWidth,decodedHeight),stage:new MaskStage(decodedWidth,decodedHeight)};
   }
   const {source,stage}=resources.current!,rect=maskPixelBounds(mask,crop);
   const last=resources.current!.last,time=image.currentTime??0;
   if(!last||last.texture!==texture||last.version!==texture.version||last.time!==time||last.mask!==mask){
    stage.update(gl,source.update(gl,texture),{rect:[rect.x*scaleX,rect.y*scaleY,rect.width*scaleX,rect.height*scaleY],
     radius:16*scaleX,opacity:1,highlightOpacity:mask.highlightOpacity,sensitive:mask.type==='sensitive-data'});
    resources.current!.last={texture,version:texture.version,time,mask};
    maskRevision.current++;
   }
   output=stage.target.texture;
   encodedMask.current.value=true;
   const view=viewRef?.current??{x:0,y:0,width:1,height:1};
   output.repeat.set(view.width,view.height);output.offset.set(view.x,1-view.y-view.height);output.updateMatrix();
  }
  if(output&&width&&height){
   const view=viewRef?.current??{x:0,y:0,width:1,height:1};
   const transform=sourceTextureTransform(output.matrix,view);
   const dimensions=output.isRenderTargetTexture?output.image as {width:number;height:number}:{width,height};
   const filteredWidth=Math.max(1,Math.round(dimensions.width*transform.elements[0]));
   const filteredHeight=Math.max(1,Math.round(dimensions.height*transform.elements[4]));
   output=filter.current.update(gl,output,filteredWidth,filteredHeight,transform,
    `${texture.version}:${image.currentTime??0}:${maskRevision.current}`);
   output.repeat.set(view.width,view.height);output.offset.set(view.x,1-view.y-view.height);output.updateMatrix();
   encodedMask.current.value=true;
  }
  if(material.current.map!==output){material.current.map=output;material.current.needsUpdate=true;}
 },-.5);
 return material;
}
