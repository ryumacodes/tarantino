export interface RecordedCursorImage {
  width:number;
  height:number;
  rgba_base64:string;
  rect:[number,number,number,number];
}
export interface RecordedCursorData {
  images:(RecordedCursorImage|null)[];
  changes:{timestamp:number;image_id:number}[];
  fallback_image_id?:number|null;
}

export function recordedCursorIndex(data:RecordedCursorData,time:number|null):number|null {
  if(time===null || !Number.isFinite(time) || time<0)return null;
  let lo=0,hi=data.changes.length;
  while(lo<hi) {
    const mid=(lo+hi)>>>1;
    if(data.changes[mid].timestamp<time)lo=mid+1;else hi=mid;
  }
  const selected=data.changes[lo]?.timestamp===time ? lo : lo-1;
  const id=data.changes[selected]?.image_id;
  if(id!==undefined && data.images[id])return id;
  const fallback=data.fallback_image_id;
  return fallback!=null && data.images[fallback] ? fallback : null;
}

// Raw DataTexture bytes use bottom-up rows; the CPU decoder uses top-down rows.
// Flip once while loading, not every playback frame or mip-pyramid update.
export function cursorTextureBytes(image:RecordedCursorImage):Uint8Array {
  const raw=atob(image.rgba_base64),stride=image.width*4;
  if(!Number.isSafeInteger(stride) || image.width<=0 || image.height<=0
    || raw.length!==stride*image.height)throw new Error('Invalid cursor bitmap');
  const bytes=new Uint8Array(raw.length);
  for(let y=0;y<image.height;y++)for(let x=0;x<stride;x++) {
    bytes[(image.height-1-y)*stride+x]=raw.charCodeAt(y*stride+x);
  }
  return bytes;
}
