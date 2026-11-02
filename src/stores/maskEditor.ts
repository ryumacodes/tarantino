import {create} from 'zustand';
/** Runtime selection/tool state; never serialized into a recording project. */
export const useMaskEditor=create<{
 selectedId:string|null;draw:boolean;select:(id:string|null)=>void;startDrawing:()=>void;finishDrawing:()=>void;
}>(set=>({selectedId:null,draw:false,
 select:selectedId=>set({selectedId,draw:false}),startDrawing:()=>set({draw:true}),finishDrawing:()=>set({draw:false}),
}));
