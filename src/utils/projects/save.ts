import { invoke } from '@tauri-apps/api/core';
import type { ProjectDocument } from './document';

// A handoff's newest snapshot must finish after any older autosave in flight.
let pending: Promise<unknown> = Promise.resolve();
export function saveProject(project: ProjectDocument, projectId: string | null): Promise<string> {
  const result = pending.then(() => invoke<string>('save_project', { project, projectId }));
  pending = result.catch(() => undefined);
  return result;
}
