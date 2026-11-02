import { invoke } from '@tauri-apps/api/core';
import { parseStylePreset, type StylePreset } from './stylePreset';
export interface PresetLibrary {version:1;revision:number;presets:StylePreset[];defaultId:string|null}
export function parsePresetLibrary(value:unknown):PresetLibrary{
  const data=value as PresetLibrary;
  if(!data || data.version!==1 || !Number.isSafeInteger(data.revision) || data.revision<0 || !Array.isArray(data.presets) || data.presets.length>1000)
    throw new Error('Invalid preset library');
  const presets=data.presets.map(parseStylePreset),ids=new Set(presets.map(p=>p.id));
  if(ids.size!==presets.length || (data.defaultId!==null && !ids.has(data.defaultId)))throw new Error('Invalid preset library references');
  return {version:1,revision:data.revision,presets,defaultId:data.defaultId};
}
export async function loadPresetLibrary(){return parsePresetLibrary(await invoke('load_style_presets'));}
export async function savePresetLibrary(library:PresetLibrary){
  return parsePresetLibrary(await invoke('save_style_presets',{library:parsePresetLibrary(library)}));
}
