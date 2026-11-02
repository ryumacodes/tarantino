import { afterEach, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), drag: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ startDragging: mocks.drag }) }));
const flush = async () => { for (let i=0; i<8; i++) await Promise.resolve(); };
async function setup() {
  vi.resetModules();
  vi.useFakeTimers();
  const nodes = Object.fromEntries(['time', 'message', 'stop', 'restart'].map(id => [id, {
    textContent: '', className: '', disabled: false,
    handlers: {} as Record<string, (event?: unknown) => void>,
    addEventListener(name: string, handler: (event?: unknown) => void) { this.handlers[name] = handler; },
  }]));
  const lifecycle: Record<string, () => void> = {};
  vi.stubGlobal('document', { getElementById: (id: string) => nodes[id] });
  vi.stubGlobal('window', { addEventListener: (name: string, handler: () => void) => { lifecycle[name] = handler; } });
  mocks.invoke.mockReset().mockResolvedValue({ is_recording: true, tray_state: { recording_duration: '00:01:23' } });
  await import('./recordingHud');
  await flush();
  return { nodes, lifecycle };
}
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });
it('uses native elapsed time and stops polling when the HUD closes', async () => {
  const { nodes, lifecycle } = await setup();
  expect(nodes.time.textContent).toBe('00:01:23');
  await vi.advanceTimersByTimeAsync(1000);
  expect(mocks.invoke).toHaveBeenCalledTimes(2);
  lifecycle.pagehide();
  await vi.advanceTimersByTimeAsync(5000);
  expect(mocks.invoke).toHaveBeenCalledTimes(2);
});
it('deduplicates Stop, exposes failure and allows a retry', async () => {
  const { nodes, lifecycle } = await setup();
  let reject!: (error: Error) => void;
  mocks.invoke.mockImplementation(command => command === 'hud_stop'
    ? new Promise((_, fail) => { reject = fail; })
    : Promise.resolve({ is_recording: true, tray_state: { recording_duration: '00:01:23' } }));
  nodes.stop.handlers.click();
  nodes.stop.handlers.click();
  expect(mocks.invoke.mock.calls.filter(([command]) => command === 'hud_stop')).toHaveLength(1);
  expect(nodes.stop.disabled).toBe(true);
  expect(nodes.restart.disabled).toBe(true);
  reject(new Error('stop unavailable'));
  await flush();
  expect(nodes.message.textContent).toContain('stop unavailable');
  expect(nodes.stop.disabled).toBe(false);
  mocks.invoke.mockResolvedValue(undefined);
  nodes.stop.handlers.click();
  await flush();
  expect(mocks.invoke.mock.calls.filter(([command]) => command === 'hud_stop')).toHaveLength(2);
  lifecycle.pagehide();
});
