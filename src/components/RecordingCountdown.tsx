import { useState } from 'react';
import { countdownChoices, readCountdown, saveCountdown } from '../utils/capture/countdown';

export function CountdownSetting() {
  const [seconds, setSeconds] = useState(readCountdown);
  const [error, setError] = useState<string | null>(null);
  return <fieldset className="capture-countdown-setting">
    <legend>Before recording</legend>
    <span>Countdown</span>
    <div role="group" aria-label="Recording countdown">
      {countdownChoices.map(value => <button key={value} type="button" aria-pressed={seconds === value}
        onClick={() => {
          try { saveCountdown(value); setSeconds(value); setError(null); }
          catch { setError('Could not save countdown preference.'); }
        }}>{value === 0 ? 'Off' : `${value}s`}</button>)}
    </div>
    {error && <p role="alert">{error}</p>}
  </fieldset>;
}

export function RecordingCountdown({ remaining, cancel }: { remaining: number | null; cancel: () => void }) {
  if (remaining === null) return null;
  return <div className="capture-countdown" role="dialog" aria-modal="true" aria-label="Recording countdown" onMouseDown={event => event.stopPropagation()}>
    <span role="status" aria-live="polite">Recording in <strong>{remaining}</strong></span>
    <button type="button" autoFocus onClick={cancel}>Cancel <small>Esc</small></button>
  </div>;
}
