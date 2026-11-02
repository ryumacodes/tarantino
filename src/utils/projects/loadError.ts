/** Native command failures arrive as strings; JavaScript failures use Error. */
export function projectLoadError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string' && error.trim()) return error;
  return 'Failed to load editor';
}
