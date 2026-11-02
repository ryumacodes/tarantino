import { beforeEach,expect,it,vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { parsePresetLibrary,loadPresetLibrary,savePresetLibrary } from './library';
import { captureStylePreset } from './stylePreset';
import { useEditorStore } from '../../stores/editor';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.mocked(invoke).mockReset();});
const preset=captureStylePreset(useEditorStore.getState(),'Reusable','one');
const library={version:1 as const,revision:3,presets:[preset],defaultId:'one'};
it('loads and saves a validated library with its concurrency revision',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce(library).mockResolvedValueOnce({...library,revision:4});
 expect(await loadPresetLibrary()).toEqual(library);
 expect((await savePresetLibrary(library)).revision).toBe(4);
 expect(invoke).toHaveBeenLastCalledWith('save_style_presets',{library});
});
it('rejects duplicate identifiers, dangling defaults and invalid preset data before saving',async()=>{
 for(const invalid of [{...library,presets:[preset,preset]},{...library,defaultId:'gone'},{...library,revision:-1},{...library,presets:[{...preset,version:2}]}]){
  expect(()=>parsePresetLibrary(invalid)).toThrow();
  await expect(savePresetLibrary(invalid as any)).rejects.toThrow();
 }
 expect(invoke).not.toHaveBeenCalled();
});
it('propagates save conflicts rather than presenting unsaved edits as persisted',async()=>{
 vi.mocked(invoke).mockRejectedValue(new Error('Reload before saving'));
 await expect(savePresetLibrary(library)).rejects.toThrow('Reload before saving');
 expect(library.revision).toBe(3);expect(library.presets[0].name).toBe('Reusable');
});
