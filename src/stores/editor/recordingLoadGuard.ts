import type { EditorState } from './types';

/** Only the newest request for the current recording session may publish a result. */
export function createRecordingLoadGuard(get: () => Pick<EditorState,'videoFilePath'|'recordingRevision'>) {
  const requests = new Map<string,number>();
  return (kind: string) => {
    const {videoFilePath,recordingRevision}=get();
    const request=(requests.get(kind) ?? 0)+1;
    requests.set(kind,request);
    return () => {
      const current=get();
      return requests.get(kind)===request && current.videoFilePath===videoFilePath
        && current.recordingRevision===recordingRevision;
    };
  };
}
