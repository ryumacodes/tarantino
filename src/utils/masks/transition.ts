import type {RecordingMask} from './document';
const LEAD_MS=200;
const BRIDGE_MS=400;
const visible=(mask:RecordingMask|null)=>!!mask&&!mask.disabled&&mask.bounds.length>0;
const sensitive=(mask:RecordingMask|null)=>visible(mask)&&mask!.type==='sensitive-data';
const inRange=(mask:RecordingMask,time:number)=>time>=mask.sourceStartMs&&time<=mask.sourceEndMs;
const near=(mask:RecordingMask|null,time:number)=>visible(mask)
  &&time>=mask!.sourceStartMs-(sensitive(mask)?LEAD_MS:0)&&time<=mask!.sourceEndMs+LEAD_MS;

/** Neighbours come from source order, including disabled entries. The active item
 * is resolved by the timeline's boundary policy before choosing transition targets.
 */
export function maskTransitionTarget(time:number,active:RecordingMask|null,previous:RecordingMask|null,next:RecordingMask|null):RecordingMask|null {
  if(!Number.isFinite(time))throw new RangeError('Invalid mask time');
  if(sensitive(active))return active;
  const bridge=!active&&visible(previous)&&visible(next)&&next!.sourceStartMs-previous!.sourceEndMs<BRIDGE_MS;
  if(bridge||(sensitive(next)&&near(next,time)))return next;
  if(visible(active))return active;
  return near(previous,time)?previous:null;
}

/** Null means snap; a number is the available spring-settling window in ms.
 * Sensitive masks snap during their active interval so motion cannot expose it.
 */
export function maskTransitionWindow(mask:RecordingMask,previous:RecordingMask|null,time:number):number|null {
  if(!Number.isFinite(time))throw new RangeError('Invalid mask time');
  if((sensitive(mask)&&inRange(mask,time))||!visible(previous))return null;
  const gap=mask.sourceStartMs-previous!.sourceEndMs;
  return gap>=BRIDGE_MS?null:gap+(sensitive(mask)?0:LEAD_MS);
}
export function maskCoverageTarget(mask:RecordingMask|null,time:number):number {
  if(!Number.isFinite(time))throw new RangeError('Invalid mask time');
  return mask&&time<=mask.sourceEndMs?1:0;
}
