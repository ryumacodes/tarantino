import type {EditorState} from '../../stores/editor/types';
import {animatedShortcutPlan} from './animatedExport';
import {KeycapAtlas} from './keycapAtlas';

export async function shortcutExportPayload(state:EditorState,width:number,height:number,fps:number) {
  if(!state.shortcutSettings.enabled||!state.shortcutIntervals.length)return undefined;
  await document.fonts.ready;
  const atlas=new KeycapAtlas(state.shortcutSettings,width,height);
  const plan=await animatedShortcutPlan(state,width,height,fps,cap=>atlas.get(cap.label).entry);
  if(!plan.motion.length)return undefined;
  const tiles=atlas.rasters.map(({canvas})=>({x:0,y:0,png:canvas.toDataURL('image/png').split(',')[1]}));
  return {width,height,frames:plan.frames,tiles,spans:[],motion:plan.motion};
}
