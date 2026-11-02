import {edit} from '../history';
import type {EditorState,EditorActions} from '../types';
import {prepareMaskAddition,prepareMaskEdit,type AddMaskInput,type MaskEdit} from '../../../utils/masks/editing';
type Store=EditorState&EditorActions;
export const createMaskActions=(set:(recipe:(state:Store)=>void)=>void,get:()=>Store)=>({
  addMask:(input:AddMaskInput)=>{
    const result=prepareMaskAddition(get().masks,input,crypto.randomUUID());
    if(result.created)edit(set,state=>{state.masks.push(result.mask);});
    return result;
  },
  updateMask:(id:string,patch:MaskEdit)=>{
    const masks=prepareMaskEdit(get().masks,id,patch);
    edit(set,state=>{state.masks=masks;});
  },
  removeMask:(id:string)=>edit(set,state=>{state.masks=state.masks.filter(mask=>mask.id!==id);}),
  applyMaskOpacityToAll:(id:string)=>{
    const mask=get().masks.find(item=>item.id===id);
    if(!mask)throw new Error('Mask no longer exists');
    edit(set,state=>{for(const item of state.masks)item.highlightOpacity=mask.highlightOpacity;});
  },
  disableAllMasks:()=>edit(set,state=>{for(const mask of state.masks)mask.disabled=true;}),
  removeAllMasks:()=>edit(set,state=>{state.masks=[];}),
});
