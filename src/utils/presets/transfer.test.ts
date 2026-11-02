import { expect,it } from 'vitest';
import { useEditorStore } from '../../stores/editor';
import { captureStylePreset } from './stylePreset';
import { importPreset,serializePreset,MAX_PRESET_BYTES } from './transfer';
it('round-trips an embedded background and gives imported presets a fresh identity',()=>{
 const state=useEditorStore.getState();
 const preset=captureStylePreset({...state,visualSettings:{...state.visualSettings,customBackgroundImage:'data:image/png;base64,aGVsbG8='}},'Portable','original');
 const text=serializePreset(preset),imported=importPreset(text,'new-id');
 expect(imported).toEqual({...preset,id:'new-id'});
 expect(preset.id).toBe('original');
});
it('rejects external asset references instead of exporting a broken or remote dependency',()=>{
 const preset=captureStylePreset(useEditorStore.getState(),'Style','id');
 for(const image of ['/tmp/background.png','https://example.com/image.png','data:text/html;base64,aGVsbG8=']){
  const invalid={...preset,visual:{...preset.visual,customBackgroundImage:image}};
  expect(()=>serializePreset(invalid)).toThrow('embedded');expect(()=>importPreset(JSON.stringify(invalid))).toThrow('embedded');
 }
});
it('rejects oversized, malformed and unsupported files',()=>{
 expect(()=>importPreset(' '.repeat(MAX_PRESET_BYTES+1))).toThrow('4 MB');
 expect(()=>importPreset('{broken')).toThrow();
 expect(()=>importPreset('{"format":"other","version":1}')).toThrow('Invalid preset');
});
