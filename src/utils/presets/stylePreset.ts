import type { AudioSettings, EditorState, ExportSettings, VisualSettings } from '../../stores/editor/types';
import { DEFAULT_VISUAL_SETTINGS, DEFAULT_EXPORT_SETTINGS } from '../../stores/editor/constants';

export interface StylePreset {
  format:'tarantino-style'; version:1; id:string; name:string;
  visual:Omit<VisualSettings,'sourceCrop'>;
  audio:Omit<AudioSettings,'backgroundAudio'>;
  output:ExportSettings;
}
const audioDefaults={micMuted:false,systemMuted:false,micGain:0,systemGain:0,noiseGate:false,dualTrack:false};
const enums:Record<string,readonly unknown[]>={
  backgroundType:['solid','gradient','wallpaper'],gradientDirection:['to-right','to-bottom','to-bottom-right','radial'],
  windowLayoutMode:['focus','desktop'],cursorStyle:['pointer','circle','filled','outline','dotted'],clickEffect:['none','circle','ripple'],
  screenMotionStyle:['focused','smooth'],cursorMotionStyle:['smooth','medium','rapid','none'],
  zoomSpeedPreset:['slow','mellow','quick','rapid'],cursorSpeedPreset:['slow','mellow','quick','rapid'],
  aspectRatio:['16:9','9:16','4:3','1:1','21:9','auto'],deviceFrame:['none','iphone-15-pro','iphone-15','ipad-pro','macbook-pro','browser'],
  webcamCorner:['top-left','top-right','bottom-left','bottom-right'],webcamLayout:['overlay','fullscreen','hidden'],webcamShape:['circle','roundrect'],
  resolution:['720p','1080p','1440p','4k','custom'],frameRate:[24,30,60],quality:['low','medium','high'],format:['mp4','mov','webm','gif'],
};
function record(value:unknown):value is Record<string,unknown>{return !!value && typeof value==='object' && !Array.isArray(value);}
function assert(condition:unknown,message:string):asserts condition{if(!condition)throw new Error(`Invalid preset: ${message}`);}
function settings(value:unknown,template:object){
  assert(record(value),'missing settings');
  const result:Record<string,unknown>={};
  for(const [key,fallback] of Object.entries(template)){
    if(key==='sourceCrop')continue;
    const entry=value[key]===undefined?fallback:value[key];
    if(entry===undefined)continue;
    if(key==='gradientStops'){
      assert(Array.isArray(entry) && entry.length>=2 && entry.length<=64,'gradient stops');
      result[key]=entry.map(stop=>{
        assert(record(stop) && typeof stop.color==='string' && typeof stop.position==='number' && Number.isFinite(stop.position) && stop.position>=0 && stop.position<=100,'gradient stop');
        return {color:stop.color,position:stop.position};
      });continue;
    }
    if(key==='webcamHideSilenceMs')assert(entry===null || (typeof entry==='number' && Number.isFinite(entry) && entry>=100 && entry<=60000),key);
    else if(enums[key])assert(enums[key].includes(entry),key);
    else if(fallback===null)assert(entry===null || typeof entry==='string',key);
    else assert(typeof entry===typeof fallback && (typeof entry!=='number' || Number.isFinite(entry)),key);
    result[key]=entry;
  }
  return result;
}
export function parseStylePreset(value:unknown):StylePreset{
  assert(record(value) && value.format==='tarantino-style' && value.version===1,'format or version');
  assert(typeof value.id==='string' && value.id.length>0 && value.id.length<=128,'identifier');
  assert(typeof value.name==='string' && value.name.trim().length>0 && value.name.length<=120,'name');
  const visual=settings(value.visual,DEFAULT_VISUAL_SETTINGS);
  // Absent legacy motion modes must stay absent rather than adopt new defaults.
  for(const key of ['screenMotionStyle','cursorMotionStyle','webcamLayout']){
    const entry=(value.visual as Record<string,unknown>)[key];
    if(entry===undefined)delete visual[key];
    else {assert(enums[key].includes(entry),key);visual[key]=entry;}
  }
  for(const key of ['webcamEnabled','zoomEffectsEnabled']) {
    const entry=(value.visual as Record<string,unknown>)[key];
    if(entry!==undefined){assert(typeof entry==='boolean',key);visual[key]=entry;}
  }
  const lead=(value.visual as Record<string,unknown>).zoomLeadInMs;
  if(lead!==undefined){assert(typeof lead==='number' && Number.isFinite(lead) && lead>=0 && lead<=2000,'zoom lead-in');visual.zoomLeadInMs=lead;}
  const output=settings(value.output,DEFAULT_EXPORT_SETTINGS);
  for(const key of ['customWidth','customHeight']){
    const entry=(value.output as Record<string,unknown>)[key];
    if(entry!==undefined){assert(typeof entry==='number' && Number.isInteger(entry) && entry>0 && entry<=16384,key);output[key]=entry;}
  }
  if(output.resolution==='custom')assert(output.customWidth && output.customHeight,'custom dimensions');
  return {format:'tarantino-style',version:1,id:value.id,name:value.name.trim(),visual:visual as StylePreset['visual'],
    audio:settings(value.audio,audioDefaults) as StylePreset['audio'],output:output as unknown as ExportSettings};
}
export function captureStylePreset(state:EditorState,name:string,id:string=crypto.randomUUID()):StylePreset{
  return parseStylePreset({format:'tarantino-style',version:1,id,name,visual:state.visualSettings,audio:state.audioSettings,output:state.exportSettings});
}
