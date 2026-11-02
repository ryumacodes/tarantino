import {initialMaskBounds,MASK_INITIAL_DURATION_MS,MASK_MIN_DURATION_MS,parseMasks,parseMaskCoordinateSpace,
  type MaskBounds,type RecordingMask} from './document';
import type {PixelCrop} from '../sourceCrop';

export interface AddMaskInput {
  sourceFilePath:string;
  sourceTimeMs:number;
  sourceDurationMs:number;
  coordinateSpace:PixelCrop;
  type:RecordingMask['type'];
  bounds?:MaskBounds[];
  highlightOpacity?:number;
}
export type AddMaskResult={mask:RecordingMask;created:boolean};

/** Source times are supplied by the visible clip resolver, never output playhead time. */
export function prepareMaskAddition(masks:readonly RecordingMask[],input:AddMaskInput,id:string):AddMaskResult {
  const {sourceTimeMs:time,sourceDurationMs:duration}=input;
  if(!Number.isFinite(time)||!Number.isFinite(duration)||time<0||time>=duration)
    throw new Error('Choose a position inside the recording to add a mask');
  const sourceMasks=masks.filter(mask=>mask.sourceFilePath===input.sourceFilePath)
    .sort((a,b)=>a.sourceStartMs-b.sourceStartMs);
  const existing=sourceMasks.find(mask=>mask.sourceStartMs<=time&&time<mask.sourceEndMs);
  if(existing)return {mask:existing,created:false};
  const next=sourceMasks.find(mask=>mask.sourceStartMs>time);
  const end=Math.min(duration,time+MASK_INITIAL_DURATION_MS,next?.sourceStartMs??duration);
  if(end-time<MASK_MIN_DURATION_MS)throw new Error('Not enough space here to add a mask');
  const coordinateSpace=parseMaskCoordinateSpace(input.coordinateSpace);
  const previous=sourceMasks[sourceMasks.length-1];
  // Preserve covered source pixels when reusing bounds drawn against a different crop.
  const previousBounds=previous?.bounds.map(rect=>({...rect,
    x:rect.x+(previous.coordinateSpace?previous.coordinateSpace.x-coordinateSpace.x:0),
    y:rect.y+(previous.coordinateSpace?previous.coordinateSpace.y-coordinateSpace.y:0)}));
  const mask=parseMasks([{id,sourceFilePath:input.sourceFilePath,sourceStartMs:time,sourceEndMs:end,
    coordinateSpace,disabled:false,type:input.type,
    bounds:input.bounds??initialMaskBounds(coordinateSpace.width,coordinateSpace.height,previousBounds),
    highlightOpacity:input.highlightOpacity??previous?.highlightOpacity??.5}])[0];
  return {mask,created:true};
}

export type MaskEdit=Partial<Pick<RecordingMask,'sourceStartMs'|'sourceEndMs'|'disabled'|'type'|'bounds'|'highlightOpacity'|'coordinateSpace'>>;
/** Validate before committing so a failed edit cannot leave partial privacy coverage. */
export function prepareMaskEdit(masks:readonly RecordingMask[],id:string,patch:MaskEdit):RecordingMask[] {
  const current=masks.find(mask=>mask.id===id);
  if(!current)throw new Error('Mask no longer exists');
  const updated=parseMasks([{...current,...patch}])[0];
  if(updated.sourceStartMs!==current.sourceStartMs||updated.sourceEndMs!==current.sourceEndMs){
    if(updated.sourceEndMs-updated.sourceStartMs<MASK_MIN_DURATION_MS)
      throw new Error('Masks must last at least half a second');
    if(masks.some(mask=>mask.id!==id&&mask.sourceFilePath===updated.sourceFilePath
      &&mask.sourceStartMs<updated.sourceEndMs&&mask.sourceEndMs>updated.sourceStartMs))
      throw new Error('Mask timing overlaps another mask');
  }
  return masks.map(mask=>mask.id===id?updated:mask);
}
