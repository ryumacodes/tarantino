import type {PixelCrop} from '../sourceCrop';
/** Rectangles use creation-crop pixels when coordinateSpace exists, otherwise current-crop pixels. */
export interface MaskBounds {x:number;y:number;width:number;height:number}
export interface RecordingMask {
  id:string;
  sourceFilePath:string;
  sourceStartMs:number;
  sourceEndMs:number;
  disabled:boolean;
  type:'sensitive-data'|'highlight';
  bounds:MaskBounds[];
  /** Pixel crop at creation; omission preserves legacy current-crop coordinates. */
  coordinateSpace?:PixelCrop;
  highlightOpacity:number;
}
export const MASK_INITIAL_DURATION_MS=3000;
export const MASK_MIN_DURATION_MS=500;
const object=(value:unknown):value is Record<string,unknown>=>!!value&&typeof value==='object'&&!Array.isArray(value);
const finite=(value:unknown):value is number=>typeof value==='number'&&Number.isFinite(value);
const identity=(value:unknown):value is string=>typeof value==='string'&&value.trim().length>0&&value.length<=4096;

/** Reject a malformed mask document rather than silently dropping privacy coverage. */
export function parseMasks(input:unknown):RecordingMask[] {
  if(input===undefined)return [];
  const invalid=()=>new Error('Invalid project: masks');
  if(!Array.isArray(input)||input.length>10000)throw invalid();
  const ids=new Set<string>();let count=0;
  return input.map(mask=>{
    if(!object(mask)||!identity(mask.id)||ids.has(mask.id)||!identity(mask.sourceFilePath)
      ||!finite(mask.sourceStartMs)||mask.sourceStartMs<0||!finite(mask.sourceEndMs)||mask.sourceEndMs<=mask.sourceStartMs
      ||typeof mask.disabled!=='boolean'||(mask.type!=='sensitive-data'&&mask.type!=='highlight')
      ||!finite(mask.highlightOpacity)||mask.highlightOpacity<.1||mask.highlightOpacity>.9
      ||!Array.isArray(mask.bounds)||!mask.bounds.length)throw invalid();
    ids.add(mask.id);count+=mask.bounds.length;if(count>100000)throw invalid();
    const bounds=mask.bounds.map(rect=>{
      if(!object(rect)||!finite(rect.x)||!finite(rect.y)||!finite(rect.width)||rect.width<=0
        ||!finite(rect.height)||rect.height<=0||!Number.isFinite(rect.x+rect.width)||!Number.isFinite(rect.y+rect.height))throw invalid();
      return {x:rect.x,y:rect.y,width:rect.width,height:rect.height};
    });
    const coordinateSpace=mask.coordinateSpace===undefined?undefined:parseMaskCoordinateSpace(mask.coordinateSpace);
    return {...(coordinateSpace?{coordinateSpace}:{}),id:mask.id,sourceFilePath:mask.sourceFilePath,sourceStartMs:mask.sourceStartMs,sourceEndMs:mask.sourceEndMs,
      disabled:mask.disabled,type:mask.type as RecordingMask['type'],bounds,highlightOpacity:mask.highlightOpacity};
  });
}

export function initialMaskBounds(width:number,height:number,previous?:readonly MaskBounds[]):MaskBounds[] {
  if(!Number.isFinite(width)||!Number.isFinite(height)||width<=0||height<=0)throw new RangeError('Invalid recording size');
  if(previous?.length)return previous.map(rect=>({...rect}));
  const w=Math.min(350,width),h=Math.min(80,height);
  return [{x:(width-w)/2,y:(height-h)/2,width:w,height:h}];
}

/** Persist quantized edges so later crop changes cannot move the covered source pixels. */
export function parseMaskCoordinateSpace(value:unknown):PixelCrop {
  if(!object(value)||![value.x,value.y,value.width,value.height,value.sourceWidth,value.sourceHeight]
    .every(n=>typeof n==='number'&&Number.isSafeInteger(n))
    ||(value.x as number)<0||(value.y as number)<0||(value.width as number)<=0||(value.height as number)<=0
    ||(value.sourceWidth as number)<=0||(value.sourceHeight as number)<=0
    ||(value.x as number)+(value.width as number)>(value.sourceWidth as number)
    ||(value.y as number)+(value.height as number)>(value.sourceHeight as number))
    throw new Error('Invalid project: masks coordinate space');
  return {x:value.x as number,y:value.y as number,width:value.width as number,height:value.height as number,
    sourceWidth:value.sourceWidth as number,sourceHeight:value.sourceHeight as number};
}
