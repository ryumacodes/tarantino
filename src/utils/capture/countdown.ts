export const countdownChoices = [0, 3, 5, 10] as const;
export type CountdownSeconds = typeof countdownChoices[number];
const key = 'tarantino.capture.countdown';
export function readCountdown(): CountdownSeconds {
  try {
    const value = Number(localStorage.getItem(key));
    return countdownChoices.includes(value as CountdownSeconds) ? value as CountdownSeconds : 0;
  } catch { return 0; }
}
export function saveCountdown(value: CountdownSeconds) {
  localStorage.setItem(key, String(value));
}

// Use a deadline so delayed browser timers do not extend the countdown.
export function waitForCountdown(seconds: CountdownSeconds, signal: AbortSignal, onTick: (remaining: number) => void): Promise<boolean> {
  if (signal.aborted) return Promise.resolve(false);
  if (seconds === 0) return Promise.resolve(true);
  return new Promise(resolve => {
    const end = performance.now() + seconds * 1000;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let last = -1;
    const finish = (ready: boolean) => {
      clearTimeout(timer);
      signal.removeEventListener('abort', cancel);
      resolve(ready);
    };
    const cancel = () => finish(false);
    const tick = () => {
      if (signal.aborted) { finish(false); return; }
      const remaining = Math.max(0, Math.ceil((end - performance.now()) / 1000));
      if (remaining !== last) { last = remaining; onTick(remaining); }
      if (remaining === 0) { finish(true); return; }
      timer = setTimeout(tick, Math.min(100, Math.max(1, end - performance.now())));
    };
    signal.addEventListener('abort', cancel, { once: true });
    tick();
  });
}
