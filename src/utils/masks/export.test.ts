import {expect,it} from 'vitest';
import {useEditorStore} from '../../stores/editor/store';
import {maskExportPayload} from './export';
import type {RecordingMask} from './document';
const mask:RecordingMask={id:'one',sourceFilePath:'a',sourceStartMs:100,sourceEndMs:600,disabled:false,type:'highlight',bounds:[{x:50,y:25,width:100,height:50}],highlightOpacity:.5};
it('normalizes cropped pixel geometry and projects exact frame intervals',()=>{
 const state={...useEditorStore.getState(),videoFilePath:'a',videoWidth:800,videoHeight:400,duration:1000,trimStart:200,trimEnd:800,
 masks:[mask],clips:[],tracks:[],visualSettings:{...useEditorStore.getState().visualSettings,sourceCrop:{x:.25,y:.25,width:.5,height:.5}}};
 expect(maskExportPayload(state,30)).toEqual({frames:18,spans:[{start:0,end:12,rect:[.125,.125,.25,.25],radius:.04,sensitive:false,highlightOpacity:.5}]});
 expect(maskExportPayload({...state,masks:[]},30)).toBeUndefined();
 expect(()=>maskExportPayload({...state,masks:[mask,{...mask,id:'two'}]},30)).toThrow('Overlapping');
});
it('keeps the same source pixels covered when a crop moves or is removed',()=>{
 const state={...useEditorStore.getState(),videoFilePath:'a',videoWidth:800,videoHeight:400,duration:1000,trimStart:0,trimEnd:1000,
  masks:[{...mask,coordinateSpace:{x:200,y:100,width:400,height:200,sourceWidth:800,sourceHeight:400}}],clips:[],tracks:[],
  visualSettings:{...useEditorStore.getState().visualSettings,sourceCrop:{x:.125,y:.125,width:.5,height:.5}}};
 const shifted=maskExportPayload(state,30)!.spans[0];
 expect(shifted.rect).toEqual([150/400,75/200,100/400,50/200]);
 const full=maskExportPayload({...state,visualSettings:{...state.visualSettings,sourceCrop:undefined}},30)!.spans[0];
 expect(full.rect).toEqual([250/800,125/400,100/800,50/400]);
 expect(full.radius).toBe(16/800);
 // Cropping past the mask keeps its location outside rather than pinning it to an edge.
 const outside=maskExportPayload({...state,visualSettings:{...state.visualSettings,sourceCrop:{x:.5,y:.5,width:.5,height:.5}}},30)!.spans[0];
 expect(outside.rect).toEqual([-150/400,-75/200,100/400,50/200]);
});
it('uses each uncropped recording size while honoring the decoder shared pixel crop',()=>{
 const state={...useEditorStore.getState(),videoFilePath:'a',videoWidth:800,videoHeight:400,duration:1000,trimStart:0,trimEnd:1000,
  masks:[{...mask,coordinateSpace:{x:0,y:0,width:1600,height:800,sourceWidth:1600,sourceHeight:800}}],clips:[],tracks:[],
  visualSettings:{...useEditorStore.getState().visualSettings,sourceCrop:undefined}};
 expect(maskExportPayload(state,30)!.spans[0].rect).toEqual([50/1600,25/800,100/1600,50/800]);
 expect(maskExportPayload({...state,visualSettings:{...state.visualSettings,sourceCrop:{x:0,y:0,width:1,height:1}}},30)!.spans[0].rect)
  .toEqual([50/800,25/400,100/800,50/400]);
});
