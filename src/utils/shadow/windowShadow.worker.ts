import {windowShadowPixels,type ShadowField} from './windowShadow';

self.onmessage=(event:MessageEvent<ShadowField>)=>{
  const pixels=windowShadowPixels(event.data);
  self.postMessage(pixels,{transfer:[pixels.buffer]});
};
