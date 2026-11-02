import {useEffect,useMemo} from 'react';
import type {WebGLRenderer,WebGLRenderTarget} from 'three';
import {CameraLayerEffect} from './CameraLayerPass';
import type {CameraPreviewSource} from './CameraPreviewSource';
class StageCameraEffect extends CameraLayerEffect {
  constructor(private source:CameraPreviewSource){super();}
  override update(_renderer:WebGLRenderer,input:WebGLRenderTarget){
    this.setFrame(input.width,input.height,this.source.frame(input.width,input.height),false);
  }
}
export function CameraPreviewEffect({source}:{source:CameraPreviewSource}) {
  const effect=useMemo(()=>new StageCameraEffect(source),[source]);
  useEffect(()=>()=>effect.dispose(),[effect]);
  return <primitive object={effect}/>;
}
