import {useEffect,useRef} from 'react';
import {useEditorStore} from '../../../stores/editor';
import {KeycapAtlas} from '../../../utils/shortcuts/keycapAtlas';
import {KeycapGpu} from '../../../utils/shortcuts/keycapGpu';
import {createShortcutMotion} from '../../../utils/shortcuts/motionTimeline';
import type {KeycapMotion} from '../../../utils/shortcuts/keycapMotion';

/** Shared output-clock animation and atlas, with GPU blur and context recovery. */
export function ShortcutPreviewOverlay() {
  const canvas=useRef<HTMLCanvasElement>(null),fallback=useRef<HTMLCanvasElement>(null);
  useEffect(()=>{
    const element=canvas.current,backup=fallback.current,ctx=backup?.getContext('2d');
    if(!element||!backup||!ctx)return;
    let gpu:KeycapGpu|undefined,lost=false;
    const initialize=()=>{
      try {gpu=new KeycapGpu(element);lost=false;}
      catch(error){gpu=undefined;console.warn('Shortcut GPU unavailable; using canvas fallback',error);}
      element.style.visibility=gpu?'visible':'hidden';backup.style.visibility=gpu?'hidden':'visible';
    };
    initialize();
    let dependencies:unknown[]=[],motion:KeycapMotion|undefined,atlas:KeycapAtlas|undefined,previousTime=NaN;
    const draw=()=>{
      const state=useEditorStore.getState(),settings=state.shortcutSettings;
      const box=element.getBoundingClientRect(),dpr=Math.min(2,window.devicePixelRatio||1);
      const width=Math.round(box.width*dpr),height=Math.round(box.height*dpr);
      const next=[settings,state.shortcutIntervals,state.clips,state.tracks,state.duration,state.trimStart,state.trimEnd,
        state.videoFilePath,state.hasWebcam,state.visualSettings,state.webcamKeyframes,state.exportSettings.frameRate,width,height];
      const changed=next.some((value,index)=>value!==dependencies[index]);
      if(changed) {
        dependencies=next;gpu?.resetAtlas();
        atlas=width>0&&height>0?new KeycapAtlas(settings,width,height):undefined;
        motion=settings.enabled&&width>0&&height>0?createShortcutMotion(state,width,height,state.exportSettings.frameRate):undefined;
      }
      if(!changed&&previousTime===state.currentTime)return;
      previousTime=state.currentTime;
      for(const surface of [element,backup]) {
        if(surface.width!==width)surface.width=width;
        if(surface.height!==height)surface.height=height;
      }
      const caps=motion?.sample(state.currentTime-state.trimStart)??[];
      if(gpu&&!lost) {if(atlas)gpu.draw(atlas,caps);else gpu.clear();}
      else {ctx.clearRect(0,0,width,height);atlas?.draw(ctx,caps);}
    };
    const onLost=(event:Event)=>{
      event.preventDefault();lost=true;element.style.visibility='hidden';backup.style.visibility='visible';previousTime=NaN;draw();
    };
    const onRestored=()=>{gpu?.destroy();initialize();previousTime=NaN;draw();};
    element.addEventListener('webglcontextlost',onLost);element.addEventListener('webglcontextrestored',onRestored);
    const observer=new ResizeObserver(draw);observer.observe(element);
    const unsubscribe=useEditorStore.subscribe(draw);draw();
    return()=>{unsubscribe();observer.disconnect();element.removeEventListener('webglcontextlost',onLost);
      element.removeEventListener('webglcontextrestored',onRestored);gpu?.destroy();};
  },[]);
  const style={position:'absolute' as const,inset:0,width:'100%',height:'100%',pointerEvents:'none' as const,zIndex:12};
  return <><canvas ref={canvas} aria-hidden="true" style={style}/><canvas ref={fallback} aria-hidden="true" style={style}/></>;
}
