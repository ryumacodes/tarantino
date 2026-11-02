import { expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { useEditorStore } from '../../stores/editor';
import { saveBeforeRecordingHandoff } from './handoff';
import { createProjectDocument } from './document';
import { saveProject } from './save';
import { invoke } from '@tauri-apps/api/core';

it('waits for edits to save before navigating and keeps the old project on failure', async () => {
  const state={...useEditorStore.getState(),videoFilePath:'/old.mp4',projectTitle:'Unsaved edits',zoomLoading:false};
  let release!:()=>void;
  const save=vi.fn((_document: unknown, _id: string|null)=>new Promise<void>(resolve=>{release=resolve;}));
  const navigate=vi.fn();
  const task=saveBeforeRecordingHandoff(state,'old-id','/editor.html?media=new',save,navigate);
  expect(save.mock.calls[0]?.[0]).toMatchObject({state:{projectTitle:'Unsaved edits',videoFilePath:'/old.mp4'}});
  expect(save.mock.calls[0]?.[1]).toBe('old-id');
  expect(navigate).not.toHaveBeenCalled();
  release(); await task;
  expect(navigate).toHaveBeenCalledWith('/editor.html?media=new');
  navigate.mockClear();
  await expect(saveBeforeRecordingHandoff(state,'old-id','new',async()=>{throw new Error('disk full');},navigate)).rejects.toThrow('disk full');
  expect(navigate).not.toHaveBeenCalled();
  expect(state.projectTitle).toBe('Unsaved edits');
});

it('serializes handoff save after autosave and recovers after write failure', async () => {
  const call=vi.mocked(invoke); call.mockReset();
  let fail!:(e:Error)=>void;
  call.mockImplementationOnce(()=>new Promise((_resolve,reject)=>{fail=reject;}));
  call.mockResolvedValueOnce('saved');
  const document=createProjectDocument({...useEditorStore.getInitialState(),videoFilePath:'/take.mp4'});
  const first=saveProject(document,'old');
  const failure=expect(first).rejects.toThrow('disk full');
  const second=saveProject(document,'new');
  await Promise.resolve();
  expect(call).toHaveBeenCalledTimes(1);
  fail(new Error('disk full')); await failure;
  expect(await second).toBe('saved');
  expect(call).toHaveBeenLastCalledWith('save_project',{project:document,projectId:'new'});
});
