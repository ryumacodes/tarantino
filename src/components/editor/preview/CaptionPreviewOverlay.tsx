import {useEffect,useRef,useState} from 'react';
import {useEditorStore} from '../../../stores/editor';
import {createCaptionRenderer} from '../../../utils/captions/renderer';

/** Render in output pixels so preview resizing cannot change caption geometry. */
export function CaptionPreviewOverlay() {
  const canvas=useRef<HTMLCanvasElement>(null);
  const [error,setError]=useState(false);
  useEffect(()=>{
    const element=canvas.current,ctx=element?.getContext('2d');if(!element||!ctx)return;
    let dependencies:unknown[]=[],previousTime=NaN,disposed=false,fontsReady=false,failed=false;
    let renderer:ReturnType<typeof createCaptionRenderer>|undefined;
    const draw=()=>{
      if(disposed)return;
      const state=useEditorStore.getState(),{width,height}=state.getExportDimensions();
      const enabled=state.captionSettings.enabled&&state.transcripts.some(session=>session.words.length>0);
      const next=[state.captionSettings,state.transcripts,state.clips,state.tracks,state.duration,state.trimStart,state.trimEnd,
        state.videoFilePath,state.exportSettings.frameRate,width,height,fontsReady];
      const changed=next.some((value,index)=>value!==dependencies[index]);
      if(!changed&&(failed||previousTime===state.currentTime))return;
      previousTime=state.currentTime;
      try {
        if(changed) {
          dependencies=next;failed=false;setError(false);renderer=undefined;
          element.width=enabled?width:1;element.height=enabled?height:1;
          if(enabled&&fontsReady&&width>0&&height>0)
            renderer=createCaptionRenderer(state,width,height,state.exportSettings.frameRate);
        }
        if(renderer)renderer.draw(ctx,state.currentTime-state.trimStart);
        else ctx.clearRect(0,0,element.width,element.height);
      }catch(cause){
        failed=true;renderer=undefined;ctx.clearRect(0,0,element.width,element.height);setError(true);
        console.error('Caption preview failed',cause);
      }
    };
    const fontsChanged=()=>{if(disposed)return;fontsReady=true;dependencies=[];draw();};
    document.fonts.ready.then(fontsChanged);
    document.fonts.addEventListener('loadingdone',fontsChanged);
    const unsubscribe=useEditorStore.subscribe(draw);draw();
    return()=>{disposed=true;unsubscribe();document.fonts.removeEventListener('loadingdone',fontsChanged);renderer=undefined;};
  },[]);
  return <><canvas ref={canvas} aria-hidden="true" style={{position:'absolute',inset:0,width:'100%',height:'100%',pointerEvents:'none',zIndex:13}}/>
    {error&&<div role="status" style={{position:'absolute',bottom:12,left:12,zIndex:20,padding:'6px 10px',background:'var(--editor-bg-secondary)',borderRadius:6}}>
      Caption preview unavailable. Try changing the caption settings or reopening the project.
    </div>}</>;
}
