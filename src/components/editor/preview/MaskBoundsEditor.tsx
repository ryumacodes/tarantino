import {useEffect,useRef,type MutableRefObject} from 'react';
import {useFrame,type ThreeEvent} from '@react-three/fiber';
import {BufferAttribute,BufferGeometry,DoubleSide,Group,LineBasicMaterial,LineSegments,Matrix4,Mesh,Plane,Vector3} from 'three';
import {useMaskEditor} from '../../../stores/maskEditor';
import {useEditorStore} from '../../../stores/editor';
import {previewSample} from '../../../utils/timeline/previewTimeline';
import {resolveSourceCrop,type PixelCrop,type SourceCrop} from '../../../utils/sourceCrop';
import {maskPixelBounds} from '../../../utils/masks/geometry';
import {draggedMaskBounds,type MaskDragMode,type MaskPoint} from '../../../utils/masks/drag';
import type {MaskBounds,RecordingMask} from '../../../utils/masks/document';

interface Selection{mask:RecordingMask;crop:PixelCrop;bounds:MaskBounds}
export function MaskBoundsEditor({viewRef}:{viewRef:MutableRefObject<SourceCrop>}){
 const surface=useRef<Mesh>(null),group=useRef<Group>(null),border=useRef<LineSegments>(null);
 const selection=useRef<Selection|null>(null);
 const drag=useRef<{selection:Selection;anchor:MaskPoint;mode:MaskDragMode;pointer:number;target:Element}|null>(null);
 const resources=useRef<{geometry:BufferGeometry;material:LineBasicMaterial}|null>(null);
 if(!resources.current){const geometry=new BufferGeometry();geometry.setAttribute('position',new BufferAttribute(new Float32Array(24),3));
  resources.current={geometry,material:new LineBasicMaterial({color:'#ff514b',depthTest:false,depthWrite:false})};}
 const finish=()=>{if(drag.current){
  const {target,pointer}=drag.current;
  if(target.hasPointerCapture?.(pointer))target.releasePointerCapture(pointer);
  drag.current=null;useEditorStore.getState().endHistoryGroup();useMaskEditor.getState().finishDrawing();
 }};
 useEffect(()=>{
  const keydown=(event:KeyboardEvent)=>{
   if(event.defaultPrevented||event.metaKey||event.ctrlKey||event.altKey||!selection.current
    ||(event.target instanceof Element&&event.target.closest('input,select,textarea,button,[contenteditable=true]')))return;
   const mask=selection.current.mask;
   if(event.key==='Enter'){event.preventDefault();finish();useMaskEditor.getState().select(null);}
   else if(event.key==='Backspace'){event.preventDefault();finish();useEditorStore.getState().removeMask(mask.id);useMaskEditor.getState().select(null);}
   else if(event.key.toLowerCase()==='d'){event.preventDefault();finish();useEditorStore.getState().updateMask(mask.id,{disabled:!mask.disabled});}
  };
  window.addEventListener('keydown',keydown);
  window.addEventListener('pointerup',finish);window.addEventListener('pointercancel',finish);window.addEventListener('blur',finish);
  return()=>{window.removeEventListener('keydown',keydown);window.removeEventListener('pointerup',finish);window.removeEventListener('pointercancel',finish);window.removeEventListener('blur',finish);
   finish();resources.current?.geometry.dispose();resources.current?.material.dispose();};
 },[]);
 useFrame(()=>{
  const state=useEditorStore.getState(),runtime=useMaskEditor.getState(),sample=previewSample(state,state.currentTime);
  const mask=runtime.selectedId?state.masks.find(item=>item.id===runtime.selectedId):undefined,path=sample.sourceFilePath??state.videoFilePath;
  const visible=mask&&mask.sourceFilePath===path&&!state.isPlaying&&sample.sourceTime!==null
   &&sample.sourceTime>=mask.sourceStartMs&&sample.sourceTime<mask.sourceEndMs;
  if(!visible||!state.videoWidth||!state.videoHeight){selection.current=null;if(group.current)group.current.visible=false;finish();return;}
  const crop=state.visualSettings.sourceCrop?resolveSourceCrop(state.visualSettings.sourceCrop,state.videoWidth,state.videoHeight):
   resolveSourceCrop({x:0,y:0,width:1,height:1},mask.coordinateSpace?.sourceWidth??state.videoWidth,mask.coordinateSpace?.sourceHeight??state.videoHeight);
  if(drag.current&&(drag.current.selection.mask.id!==mask.id||!state.historyGroup))finish();
  const bounds=maskPixelBounds(mask,crop);selection.current={mask,crop,bounds};
  if(group.current)group.current.visible=true;
  const view=viewRef.current;
  const x=(bounds.x/crop.width-view.x)/view.width-.5,y=.5-(bounds.y/crop.height-view.y)/view.height;
  const w=bounds.width/crop.width/view.width,h=bounds.height/crop.height/view.height;
  const positions=[x,y,.004,x+w,y,.004,x+w,y,.004,x+w,y-h,.004,x+w,y-h,.004,x,y-h,.004,x,y-h,.004,x,y,.004];
  const attribute=resources.current!.geometry.getAttribute('position') as BufferAttribute;
  attribute.array.set(positions);attribute.needsUpdate=true;resources.current!.geometry.computeBoundingSphere();
 },-.25);
 const point=(event:ThreeEvent<PointerEvent>,crop:PixelCrop):MaskPoint|null=>{
  if(!surface.current)return null;
  const inverse=new Matrix4().copy(surface.current.matrixWorld).invert();
  const local=event.ray.clone().applyMatrix4(inverse).intersectPlane(new Plane(new Vector3(0,0,1),0),new Vector3());
  if(!local)return null;
  const view=viewRef.current;
  return {x:(view.x+(local.x+.5)*view.width)*crop.width,y:(view.y+(.5-local.y)*view.height)*crop.height};
 };
 const down=(event:ThreeEvent<PointerEvent>)=>{
  const selected=selection.current;if(!selected||event.button!==0)return;
  const position=point(event,selected.crop);if(!position)return;
  const rect=selected.bounds,view=viewRef.current;
  const toleranceX=selected.crop.width*view.width*.025,toleranceY=selected.crop.height*view.height*.025;
  const left=Math.abs(position.x-rect.x)<toleranceX,right=Math.abs(position.x-rect.x-rect.width)<toleranceX;
  const top=Math.abs(position.y-rect.y)<toleranceY,bottom=Math.abs(position.y-rect.y-rect.height)<toleranceY;
  let mode:MaskDragMode='move';
  if(useMaskEditor.getState().draw)mode='draw';
  else if((left||right)&&(top||bottom))mode=`${top?'n':'s'}${left?'w':'e'}`;
  else if(position.x<rect.x||position.x>rect.x+rect.width||position.y<rect.y||position.y>rect.y+rect.height)return;
  event.stopPropagation();const target=event.target as Element;target.setPointerCapture(event.pointerId);
  useEditorStore.getState().beginHistoryGroup();drag.current={selection:selected,anchor:position,mode,pointer:event.pointerId,target};
 };
 const move=(event:ThreeEvent<PointerEvent>)=>{
  const active=drag.current;if(!active)return;event.stopPropagation();
  const position=point(event,active.selection.crop);if(!position)return;
  const {mask,crop,bounds}=active.selection;
  const next=draggedMaskBounds(bounds,active.anchor,position,active.mode,crop.width,crop.height);
  // Convert the new first rectangle back to its stored creation crop; other regions retain their basis.
  const basis=mask.coordinateSpace;
  next.x-=basis?basis.x-crop.x:0;next.y-=basis?basis.y-crop.y:0;
  useEditorStore.getState().updateMask(mask.id,{bounds:[next,...mask.bounds.slice(1)]});
 };
 return <group ref={group} visible={false}>
  <mesh ref={surface} position={[0,0,.003]} onPointerDown={down} onPointerMove={move}
   onPointerUp={event=>{if(drag.current){event.stopPropagation();finish();}}} onPointerCancel={finish}>
   <planeGeometry args={[1,1]}/><meshBasicMaterial transparent opacity={0} depthWrite={false} side={DoubleSide}/>
  </mesh>
  <points geometry={resources.current.geometry} renderOrder={101}><pointsMaterial color="#ff514b" size={6} sizeAttenuation={false} depthTest={false} depthWrite={false}/></points>
  <lineSegments ref={border} geometry={resources.current.geometry} material={resources.current.material} renderOrder={100}/>
 </group>;
}
