import { useEffect, useRef, useState } from 'react';
import { readCountdown, waitForCountdown } from '../utils/capture/countdown';

export function useRecordingCountdown() {
  const [remaining, setRemaining] = useState<number | null>(null);
  const active = useRef<AbortController | null>(null);
  const cancel = () => active.current?.abort();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Tab' && active.current) event.preventDefault();
      if (event.key === 'Escape' && active.current) {
        event.preventDefault();
        active.current.abort();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => { document.removeEventListener('keydown', onKey); active.current?.abort(); };
  }, []);
  const start = async () => {
    active.current?.abort();
    const controller = new AbortController();
    active.current = controller;
    try { return await waitForCountdown(readCountdown(), controller.signal, setRemaining); }
    finally {
      if (active.current === controller) { active.current = null; setRemaining(null); }
    }
  };
  return { remaining, start, cancel };
}
