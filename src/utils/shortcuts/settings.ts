export interface ShortcutSettings {
  enabled:boolean;
  size:number;
  x:number;
  y:number;
  hidden:number[];
}
export const DEFAULT_SHORTCUT_SETTINGS:ShortcutSettings={enabled:true,size:0.065,x:0.5,y:0.9,hidden:[]};
export function parseShortcutSettings(input:unknown):ShortcutSettings {
  if(input===undefined)return {...DEFAULT_SHORTCUT_SETTINGS,hidden:[]};
  const value=input as Partial<ShortcutSettings>|null;
  if(!value || typeof value!=='object' || typeof value.enabled!=='boolean'
    || typeof value.size!=='number' || !Number.isFinite(value.size) || value.size<0.03 || value.size>0.15
    || typeof value.x!=='number' || !Number.isFinite(value.x) || value.x<0 || value.x>1
    || typeof value.y!=='number' || !Number.isFinite(value.y) || value.y<0 || value.y>1
    || !Array.isArray(value.hidden) || !value.hidden.every(n=>typeof n==='number'&&Number.isFinite(n)&&n>=0))
    throw new Error('Invalid project: shortcut settings');
  return {enabled:value.enabled,size:value.size,x:value.x,y:value.y,hidden:[...new Set(value.hidden)]};
}
