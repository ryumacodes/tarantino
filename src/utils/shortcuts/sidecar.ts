import { shortcutIntervals, type ShortcutEvent, type ShortcutModifier, type ModifierObservation } from './intervals';
const allowed:readonly string[]=['control','option','shift','command','capsLock','function'];

/** Only explicitly captured shortcut payloads may become labels. */
export function shortcutsFromSidecar(input:unknown) {
  if(!Array.isArray(input))return [];
  const events:ShortcutEvent[]=[],observations:ModifierObservation[]=[];
  for(const entry of input) {
    if(!entry || typeof entry.timestamp!=='number' || !Number.isFinite(entry.timestamp) || entry.timestamp<0)continue;
    const data=entry.shortcut;
    if(!data || !Array.isArray(data.modifiers) || !data.modifiers.every((x:unknown)=>typeof x==='string'&&allowed.includes(x)))continue;
    const modifiers=data.modifiers as ShortcutModifier[];
    if(data.key===null) {observations.push({timestamp:entry.timestamp,modifiers});continue;}
    if(typeof data.key!=='string' || !data.key.length || data.key.length>32 || /[\u0000-\u001f\u007f]/.test(data.key))continue;
    events.push({timestamp:entry.timestamp,key:data.key,modifiers,
      type:entry.is_release===true?'up':'down',repeat:data.repeat===true});
  }
  return shortcutIntervals(events,observations);
}
