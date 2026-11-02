import { useSyncExternalStore } from 'react';

const KEY = 'tarantino-typing-speed';
const DEFAULT_RATE = 3;
let sessionRate = DEFAULT_RATE;
let storageFailed = false;
const listeners = new Set<() => void>();
const valid = (value: number) => Number.isFinite(value) && value >= 1.5 && value <= 8;
function read() {
  if (storageFailed) return sessionRate;
  try {
    const saved = Number(localStorage.getItem(KEY));
    return valid(saved) ? saved : sessionRate;
  } catch { return sessionRate; }
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  const storage = (event: StorageEvent) => { if(event.key === KEY || event.key === null) listener(); };
  window.addEventListener('storage', storage);
  return () => { listeners.delete(listener); window.removeEventListener('storage',storage); };
}
function setRate(value: number) {
  if (!valid(value)) return;
  sessionRate = value;
  try { localStorage.setItem(KEY,String(value)); storageFailed = false; } catch { storageFailed = true; }
  listeners.forEach(listener => listener());
}

/** Keep sidebar and timeline controls in sync without adding preferences to project history. */
export function useTypingSpeedPreference() {
  const rate = useSyncExternalStore(subscribe, read, () => DEFAULT_RATE);
  return [rate, setRate] as const;
}
