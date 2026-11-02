import { useEditorStore } from '../../stores/editor';

export function droppedAudioPath(paths: string[]) {
  return paths.find(path=>/\.(mp3|wav|m4a|aac|ogg|opus|flac|aif|aiff)$/i.test(path));
}

/** Commit only to the unchanged recording and music selection that started the import. */
export async function importDroppedAudio(
  path: string,
  load: (path:string)=>Promise<{path:string;name:string}>,
  canApply: ()=>boolean,
) {
  const before=useEditorStore.getState();
  const revision=before.recordingRevision;
  const music=before.audioSettings.backgroundAudio;
  const asset=await load(path);
  const current=useEditorStore.getState();
  if(!canApply() || current.recordingRevision!==revision || current.audioSettings.backgroundAudio!==music)return false;
  current.updateAudioSettings({backgroundAudio:{...asset,volume:music?.volume ?? 1,muted:false}});
  return true;
}
