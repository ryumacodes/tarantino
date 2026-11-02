import { beforeEach, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({ effects: [] as (() => void | (() => void))[], listen: vi.fn() }));
vi.mock('react', () => ({
  useEffect: (effect: () => void | (() => void)) => mocks.effects.push(effect),
  useRef: (current: unknown) => ({ current }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
import { useCaptureShortcuts } from './useCaptureShortcuts';

const options = () => ({
  isRecording: false, isStarting: false, showShortcuts: false, showSpeakerNotes: false,
  onShowCaptureControls: vi.fn(), onChooseDisplay: vi.fn(), onChooseWindow: vi.fn(), onOpenSettings: vi.fn(), onOpenShortcuts: vi.fn(),
  onToggleShortcuts: vi.fn(), onCloseShortcuts: vi.fn(), onToggleSpeakerNotes: vi.fn(),
  onCloseSpeakerNotes: vi.fn(), onFinishRecording: vi.fn(), onStartRecording: vi.fn(), onRestartRecording: vi.fn(),
});
beforeEach(() => { mocks.effects.length = 0; mocks.listen.mockReset(); });

it('routes menu actions to target selection without starting an unconfirmed recording', async () => {
  mocks.listen.mockResolvedValue(vi.fn());
  const callbacks = options();
  useCaptureShortcuts(callbacks);
  const cleanup = mocks.effects[0]();
  const receive = mocks.listen.mock.calls[0][1];
  for (const payload of ['new', 'display', 'window', 'settings', 'shortcuts', 'unknown']) await receive({ payload });
  expect(callbacks.onShowCaptureControls).toHaveBeenCalledOnce();
  expect(callbacks.onChooseDisplay).toHaveBeenCalledOnce();
  expect(callbacks.onChooseWindow).toHaveBeenCalledOnce();
  expect(callbacks.onOpenSettings).toHaveBeenCalledOnce();
  expect(callbacks.onOpenShortcuts).toHaveBeenCalledOnce();
  expect(callbacks.onStartRecording).not.toHaveBeenCalled();
  if (cleanup) cleanup();
});

it.each(['isRecording', 'isStarting'] as const)('ignores stale idle-menu actions while %s', async busy => {
  mocks.listen.mockResolvedValue(vi.fn());
  const callbacks = { ...options(), [busy]: true };
  useCaptureShortcuts(callbacks);
  const cleanup = mocks.effects[0]();
  const receive = mocks.listen.mock.calls[0][1];
  await receive({ payload: 'display' });
  await receive({ payload: 'window' });
  expect(callbacks.onChooseDisplay).not.toHaveBeenCalled();
  expect(callbacks.onChooseWindow).not.toHaveBeenCalled();
  if (cleanup) cleanup();
});

it('removes a listener that finishes registering after unmount and ignores queued events', async () => {
  let resolve!: (cleanup: () => void) => void;
  mocks.listen.mockReturnValue(new Promise<() => void>(done => { resolve = done; }));
  const callbacks = options();
  useCaptureShortcuts(callbacks);
  const cleanup = mocks.effects[0]();
  if (cleanup) cleanup();
  const unlisten = vi.fn();
  resolve(unlisten);
  await Promise.resolve();
  expect(unlisten).toHaveBeenCalledOnce();
  await mocks.listen.mock.calls[0][1]({ payload: 'settings' });
  expect(callbacks.onOpenSettings).not.toHaveBeenCalled();
});
