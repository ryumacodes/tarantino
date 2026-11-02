import type {CaptionMotionOptions} from './targets';
export interface CaptionSettings extends CaptionMotionOptions {
  enabled:boolean;showSoundLabels:boolean;
  font:'serif'|'sans-serif'|'mono';sizeRatio:number;
  color:string;backgroundColor:string;position:{x:number;y:number};
}
export const DEFAULT_CAPTION_SETTINGS:CaptionSettings={
  enabled:true,showSoundLabels:false,font:'sans-serif',sizeRatio:.05,color:'#ffffff',backgroundColor:'#000000',
  position:{x:.5,y:1},wordsReveal:'line-by-line',lineGrowth:'left-anchored',wordEntrance:'appear',
};
const unit=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>=0&&n<=1;
const color=(value:unknown):value is string=>typeof value==='string'&&/^#[\da-f]{6}([\da-f]{2})?$/i.test(value);
/** The renderer receives finite bounded values and explicit supported modes. */
export function parseCaptionSettings(input:unknown):CaptionSettings {
  if(input===undefined)return {...DEFAULT_CAPTION_SETTINGS,position:{...DEFAULT_CAPTION_SETTINGS.position}};
  const s=input as Partial<CaptionSettings>|null;
  if(!s||typeof s!=='object'||Array.isArray(s)||typeof s.enabled!=='boolean'||typeof s.showSoundLabels!=='boolean'
    ||!['serif','sans-serif','mono'].includes(s.font??'')
    ||typeof s.sizeRatio!=='number'||!Number.isFinite(s.sizeRatio)||s.sizeRatio<.02||s.sizeRatio>.125
    ||!color(s.color)||!color(s.backgroundColor)||!s.position||!unit(s.position.x)||!unit(s.position.y)
    ||!['line-by-line','word-by-word'].includes(s.wordsReveal??'')
    ||!['entire-line','left-anchored','centered'].includes(s.lineGrowth??'')
    ||!['appear','fade-in','slide-in'].includes(s.wordEntrance??''))throw new Error('Invalid project: caption settings');
  return {enabled:s.enabled,showSoundLabels:s.showSoundLabels,font:s.font!,sizeRatio:s.sizeRatio,
    color:s.color,backgroundColor:s.backgroundColor,position:{x:s.position.x,y:s.position.y},
    wordsReveal:s.wordsReveal!,lineGrowth:s.lineGrowth!,wordEntrance:s.wordEntrance!};
}
