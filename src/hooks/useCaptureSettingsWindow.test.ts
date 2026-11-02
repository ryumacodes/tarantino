import { afterEach, expect, it, vi } from 'vitest';
import { LogicalSize, PhysicalSize } from '@tauri-apps/api/dpi';
const mocks = vi.hoisted(() => ({ effects: [] as (() => (() => void) | void)[], size: vi.fn(), setSize: vi.fn() }));
vi.mock('react', () => ({ useEffect: (effect: () => (() => void) | void) => mocks.effects.push(effect) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({
  innerSize: mocks.size, scaleFactor: async () => 2, setSize: mocks.setSize,
}) }));
import { useCaptureSettingsWindow } from './useCaptureSettingsWindow';

const flush = async () => { for (let i = 0; i < 10; i++) await Promise.resolve(); };
function setup() {
  mocks.effects.length = 0;
  mocks.size.mockReset().mockResolvedValue(new PhysicalSize(1420, 120));
  mocks.setSize.mockReset().mockResolvedValue(undefined);
  let notify = () => {};
  const observe = vi.fn();
  const disconnect = vi.fn();
  class Element {
    height = 240;
    getBoundingClientRect() { return { height: this.height }; }
  }
  vi.stubGlobal('HTMLElement', Element);
  vi.stubGlobal('screen', { availHeight: 956 });
  vi.stubGlobal('getComputedStyle', () => ({ paddingTop: '8px', paddingBottom: '8px', borderTopWidth: '1px', borderBottomWidth: '1px' }));
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { notify = callback; }
    observe = observe;
    disconnect = disconnect;
  });
  const content = new Element();
  // The outer scroll viewport stays fixed while its content grows/shrinks.
  const dropdown = { firstElementChild: content, scrollHeight: 285 };
  const ref = { current: { closest: () => dropdown } as unknown as HTMLElement };
  useCaptureSettingsWindow(ref);
  const cleanup = mocks.effects[0]();
  return { content, observe, disconnect, notify: () => notify(), cleanup };
}
afterEach(() => vi.unstubAllGlobals());

it('fits expanding and collapsing content even when the scroll viewport does not resize', async () => {
  const panel = setup();
  await flush();
  expect(panel.observe.mock.calls[0]?.[0] === panel.content).toBe(true);
  expect(mocks.setSize).toHaveBeenLastCalledWith(new LogicalSize(710, 346));
  panel.content.height = 480;
  panel.notify();
  await flush();
  expect(mocks.setSize).toHaveBeenLastCalledWith(new LogicalSize(710, 586));
  panel.content.height = 1200;
  panel.notify();
  await flush();
  expect(mocks.setSize).toHaveBeenLastCalledWith(new LogicalSize(710, 856));
  panel.content.height = 240;
  panel.notify();
  await flush();
  expect(mocks.setSize).toHaveBeenLastCalledWith(new LogicalSize(710, 346));
  panel.cleanup?.();
  await flush();
  expect(panel.disconnect).toHaveBeenCalledOnce();
  expect(mocks.setSize).toHaveBeenLastCalledWith(new LogicalSize(710, 60));
});

it('does not expand a panel closed before native size lookup completes', async () => {
  const panel = setup();
  panel.cleanup?.();
  await flush();
  expect(panel.observe).not.toHaveBeenCalled();
  expect(mocks.setSize).not.toHaveBeenCalled();
});
