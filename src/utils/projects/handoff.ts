import type { EditorState } from '../../stores/editor/types';
import { createProjectDocument, type ProjectDocument } from './document';

/** Do not replace a loaded document until its current edits are durable. */
export async function saveBeforeRecordingHandoff(
  state: EditorState,
  projectId: string | null,
  destination: string,
  save: (document: ProjectDocument, id: string | null) => Promise<unknown>,
  navigate: (destination: string) => void,
) {
  if (state.videoFilePath) {
    if (state.zoomLoading) throw new Error('The current project is still loading.');
    await save(createProjectDocument(state), projectId);
  }
  navigate(destination);
}
