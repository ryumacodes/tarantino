import { parseStylePreset, type StylePreset } from './stylePreset';
export const MAX_PRESET_BYTES=4*1024*1024;
function portable(preset:StylePreset){
  const image=preset.visual.customBackgroundImage;
  if(image && !/^data:image\/(png|jpeg|webp|gif|avif|bmp);base64,[A-Za-z0-9+/]+={0,2}$/.test(image))
    throw new Error('Choose the custom background image again before sharing this preset. It must be embedded in the preset file.');
  return preset;
}
export function serializePreset(preset:StylePreset){
  const validated=portable(parseStylePreset(preset));
  const text=JSON.stringify(validated,null,2);
  if(new TextEncoder().encode(text).length>MAX_PRESET_BYTES)throw new Error('Preset files must be 4 MB or smaller. Use a smaller background image.');
  return text;
}
export function importPreset(text:string,id:string=crypto.randomUUID()){
  if(new TextEncoder().encode(text).length>MAX_PRESET_BYTES)throw new Error('Preset files must be 4 MB or smaller.');
  const preset=portable(parseStylePreset(JSON.parse(text)));
  return {...preset,id};
}
