import { useEditorStore } from '../../stores/editor';
import { loadPresetLibrary, type PresetLibrary } from './library';
import { parseStylePreset } from './stylePreset';

/** Called only for a newly initialized recording, never while restoring a saved project. */
export async function applyDefaultPreset(load:()=>Promise<PresetLibrary>=loadPresetLibrary){
  const before=useEditorStore.getState();
  if(!before.videoFilePath)return false;
  const library=await load();
  const saved=library.presets.find(p=>p.id===library.defaultId);
  if(!saved)return false;
  const preset=parseStylePreset(saved);
  const current=useEditorStore.getState();
  if(current.recordingRevision!==before.recordingRevision || current.videoFilePath!==before.videoFilePath
    || current.history!==before.history || current.visualSettings!==before.visualSettings
    || current.audioSettings!==before.audioSettings || current.exportSettings!==before.exportSettings)return false;
  useEditorStore.setState({
    visualSettings:{...preset.visual,sourceCrop:current.visualSettings.sourceCrop},
    audioSettings:{...preset.audio,backgroundAudio:current.audioSettings.backgroundAudio},
    exportSettings:preset.output,
    history:[],historyIndex:-1,historyGroup:null,canUndo:false,canRedo:false,
    isPlaying:false,playbackRevision:current.playbackRevision+1,
  });
  return true;
}
