/** Source-time shortcut display intervals. Ordinary typing activity cannot be
 * converted into labels; callers must supply explicitly captured shortcut data. */
export type ShortcutModifier='control'|'option'|'shift'|'command'|'capsLock'|'function';
export interface ShortcutEvent {
  timestamp:number;
  type:'down'|'up';
  key:string;
  modifiers:readonly ShortcutModifier[];
  repeat?:boolean;
}
export interface ModifierObservation {timestamp:number;modifiers:readonly ShortcutModifier[]}
export interface ShortcutInterval {
  key:string;
  modifiers:ShortcutModifier[];
  firstDown:number;
  lastDown:number;
  lastEvent:number;
  start:number;
  end:number;
  presses:number;
}
const modifierOrder:ShortcutModifier[]=['control','option','shift','command','capsLock','function'];
const standalone=new Set(['§','CapsLock','Backspace','Delete','ArrowDown','ArrowLeft','ArrowRight','ArrowUp',
  'End','Home','PageDown','PageUp',...Array.from({length:20},(_,i)=>`F${i+1}`),
  'Soft1','Soft2','Soft3','Soft4']);
const typingKeys=new Set([..."abcdefghijklmnopqrstuvwxyz0123456789",
  'Decimal','Key11','Key12','Multiply','Add','Clear','Divide','Subtract','Separator']);
const letter=(key:string)=>typingKeys.has(key);
const noHidden:ReadonlySet<number>=new Set();
const modifiers=(keys:readonly ShortcutModifier[])=>modifierOrder.filter(key=>keys.includes(key));

export function shortcutIntervals(input:readonly ShortcutEvent[],
  observations:readonly ModifierObservation[]=[],showSingleKeys=false):ShortcutInterval[] {
  const events=input.filter(event=>Number.isFinite(event.timestamp)&&event.timestamp>=0&&event.key.length>0)
    .map(event=>({...event,modifiers:modifiers(event.modifiers)})).sort((a,b)=>a.timestamp-b.timestamp);
  const releases=new Map<ShortcutEvent,number>(),held=new Map<string,ShortcutEvent>();
  for(const event of events) {
    const identity=event.key.toLowerCase();
    if(event.type==='down') {if(!event.repeat)held.set(identity,event);}
    else {const down=held.get(identity);if(down){releases.set(down,event.timestamp);held.delete(identity);}}
  }
  const downs=events.filter(event=>event.type==='down');
  const accepted=downs.filter((event,index)=>{
    const next=downs[index+1];
    // Shift followed by ordinary text is typing, not a shortcut demonstration.
    if(event.modifiers.includes('shift')&&next&&!next.modifiers.length
      &&next.timestamp-event.timestamp<1000)return false;
    return showSingleKeys||event.modifiers.length>0||standalone.has(event.key);
  });
  const groups:ShortcutEvent[][]=[];
  for(const event of accepted) {
    const previous=groups[groups.length-1],first=previous?.[0],last=previous?.[previous.length-1];
    if(showSingleKeys&&first&&!first.modifiers.length&&!event.modifiers.length
      &&letter(first.key)&&letter(event.key)&&event.timestamp-first.timestamp<1000) {groups.pop();continue;}
    if(last&&event.timestamp-last.timestamp<=2000&&event.key===last.key
      &&event.modifiers.join(',')===last.modifiers.join(','))previous!.push(event);
    else groups.push([event]);
  }
  const samples:ModifierObservation[]=[...events,...observations]
    .filter(sample=>Number.isFinite(sample.timestamp)&&sample.timestamp>=0)
    .sort((a,b)=>a.timestamp-b.timestamp);
  const result:ShortcutInterval[]=[];
  for(const group of groups) {
    const first=group[0],last=group[group.length-1],previous=result[result.length-1];
    const lead=first.timestamp-100;
    let start=lead;
    const earliest=Math.max(first.timestamp-3000,previous?previous.lastEvent+1:-Infinity);
    // Find the last modifier observation strictly preceding this key-down.
    let low=0,high=samples.length;
    while(low<high){const middle=(low+high)>>>1;if(samples[middle].timestamp<first.timestamp)low=middle+1;else high=middle;}
    for(const modifier of first.modifiers) {
      if(modifier==='capsLock')continue;
      let heldSince=first.timestamp;
      for(let i=low-1;i>=0;i--) {
        const sample=samples[i];
        if(sample.timestamp<earliest||!sample.modifiers.includes(modifier))break;
        heldSince=sample.timestamp;
      }
      start=Math.min(start,heldSince-100);
    }
    if(previous)start=Math.min(lead,Math.max(start,previous.start+800));
    result.push({key:first.key,modifiers:[...first.modifiers],firstDown:first.timestamp,
      lastDown:last.timestamp,lastEvent:group.reduce((latest,event)=>Math.max(latest,releases.get(event)??event.timestamp),first.timestamp),
      start,end:last.timestamp+1500,presses:group.length});
  }
  return result;
}

/** Later shortcuts replace earlier overlapping displays; end times are inclusive. */
export function shortcutAt(intervals:readonly ShortcutInterval[],sourceTime:number,
  hidden:ReadonlySet<number>=noHidden):ShortcutInterval|null {
  if(!Number.isFinite(sourceTime))return null;
  let low=0,high=intervals.length;
  while(low<high) {
    const middle=(low+high)>>>1;
    if(intervals[middle].start<=sourceTime)low=middle+1;else high=middle;
  }
  let index=low-1;
  while(index>=0&&hidden.has(intervals[index].firstDown))index--;
  const current=intervals[index];
  return current&&sourceTime<=current.end?current:null;
}
