import React, { useEffect, useState } from 'react';
import * as THREE from 'three';
import type { VisualSettings } from '../../../stores/editor';


interface VideoShadowProps {
  canvasWidth: number;
  canvasHeight: number;
  inputWidth: number;
  inputHeight: number;
  outputWidth: number;
  outputHeight: number;
  settings: VisualSettings;
  textureRef?: React.MutableRefObject<THREE.Texture|null>;
}

export const VideoShadow: React.FC<VideoShadowProps> = ({canvasWidth,canvasHeight,inputWidth,inputHeight,outputWidth,outputHeight,settings,textureRef}) => {
  const {shadowEnabled,shadowIntensity,shadowBlur,shadowOffsetX,shadowOffsetY,cornerRadius,padding}=settings;
  const [shadowTexture,setShadowTexture]=useState<THREE.DataTexture|null>(null);
  useEffect(()=>{
    setShadowTexture(null);
    if(!shadowEnabled)return;
    const worker=new Worker(new URL('../../../utils/shadow/windowShadow.worker.ts',import.meta.url),{type:'module'});
    worker.onmessage=(event:MessageEvent<Uint8Array>)=>{
      const texture=new THREE.DataTexture(event.data,outputWidth,outputHeight);
      texture.flipY=true;
      texture.minFilter=texture.magFilter=THREE.LinearFilter;
      texture.needsUpdate=true;
      setShadowTexture(texture);
      worker.terminate();
    };
    worker.onerror=error=>{console.error('Preview shadow generation failed:',error.message);worker.terminate();};
    const inset=Math.max(0,Math.min(49,padding))/100;
    const contentWidth=outputWidth-2*Math.floor(outputWidth*inset);
    const contentHeight=outputHeight-2*Math.floor(outputHeight*inset);
    const width=inputWidth,height=inputHeight;
    const x=Math.floor((outputWidth-width)/2),y=Math.floor((outputHeight-height)/2);
    worker.postMessage({width:outputWidth,height:outputHeight,
      rect:{x:x+Math.trunc(shadowOffsetX),y:y+Math.trunc(shadowOffsetY),width,height},
      radius:cornerRadius>0?Math.max(1,Math.min(contentWidth,contentHeight)*cornerRadius/200):0,
      blur:shadowBlur,intensity:shadowIntensity/100});
    return ()=>{worker.onmessage=null;worker.terminate();};
  },[shadowEnabled,shadowIntensity,shadowBlur,shadowOffsetX,shadowOffsetY,cornerRadius,padding,inputWidth,inputHeight,outputWidth,outputHeight]);
  useEffect(()=>()=>shadowTexture?.dispose(),[shadowTexture]);
  useEffect(()=>{
    if(textureRef)textureRef.current=shadowTexture;
    return ()=>{if(textureRef)textureRef.current=null;};
  },[shadowTexture,textureRef]);
  if(!shadowTexture)return null;
  return <mesh position={[0,0,-.1]}>
    <planeGeometry args={[canvasWidth,canvasHeight]} />
    <meshBasicMaterial map={shadowTexture} transparent toneMapped={false} depthWrite={false} />
  </mesh>;
};
