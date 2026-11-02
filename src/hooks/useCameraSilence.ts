import { useEffect,useState } from 'react';
import { useEditorStore } from '../stores/editor';
import { loadCameraSilence,type SilenceRange } from '../utils/camera/silence';
export function useCameraSilence(){
  const path=useEditorStore(s=>s.videoFilePath),revision=useEditorStore(s=>s.recordingRevision);
  const minimum=useEditorStore(s=>s.visualSettings.webcamHideSilenceMs);
  const mic=useEditorStore(s=>s.hasMicrophone),camera=useEditorStore(s=>s.hasWebcam);
  const key=JSON.stringify([path,revision,minimum,mic,camera]);
  const [result,setResult]=useState<{key:string;ranges:SilenceRange[];error:string|null}|null>(null);
  useEffect(()=>{
    let active=true;
    void loadCameraSilence(useEditorStore.getState()).then(ranges=>{if(active)setResult({key,ranges,error:null});})
      .catch(error=>{if(active)setResult({key,ranges:[],error:String(error)});});
    return ()=>{active=false;};
  },[key]);
  return result?.key===key?{...result,loading:false}:{ranges:[],error:null,loading:minimum!=null && mic && camera};
}
