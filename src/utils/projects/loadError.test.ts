import { expect, it } from 'vitest';
import { projectLoadError } from './loadError';
it('preserves native missing-recording errors so the recovery action remains available',()=>{
  const message='Recording is missing: /moved/take.mp4. Restore it to this location before reopening.';
  expect(projectLoadError(message)).toBe(message);
  expect(projectLoadError(new Error(message))).toBe(message);
  expect(projectLoadError(null)).toBe('Failed to load editor');
  expect(projectLoadError('')).toBe('Failed to load editor');
});
