import {useEditorStore} from './store';
import {parseTranscripts,type TranscriptWord,type TranscriptGenerator} from '../../utils/captions/document';

/** Recognition runs asynchronously; never attach its result to a different project. */
export function applyGeneratedTranscript(origin:{recordingRevision:number;videoFilePath:string|null},sourceFilePath:string,words:TranscriptWord[],generator?:TranscriptGenerator):boolean {
  const state=useEditorStore.getState();
  if(state.recordingRevision!==origin.recordingRevision||state.videoFilePath!==origin.videoFilePath)return false;
  if(state.videoFilePath!==sourceFilePath&&!state.clips.some(clip=>clip.sourceFilePath===sourceFilePath))return false;
  const previous=state.transcripts.find(session=>session.sourceFilePath===sourceFilePath);
  const [session]=parseTranscripts([{id:previous?.id??crypto.randomUUID(),sourceFilePath,offsetMs:0,words,generator}]);
  state.setTranscriptSession(session);
  return true;
}
