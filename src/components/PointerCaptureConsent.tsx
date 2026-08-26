import React, { useEffect } from 'react';

interface PointerCaptureConsentProps {
  onEnable: () => void;
  onSkip: () => void;
}

const PointerCaptureConsent: React.FC<PointerCaptureConsentProps> = ({ onEnable, onSkip }) => {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onSkip();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onSkip]);

  return (
    <div className="shortcut-backdrop" onClick={onSkip}>
      <div
        className="shortcut-modal pointer-consent"
        role="dialog"
        aria-modal="true"
        aria-labelledby="pointer-consent-title"
        onClick={(event) => event.stopPropagation()}
      >
        <header>
          <h2 id="pointer-consent-title">Enable automatic zoom?</h2>
        </header>
        <p>Tarantino needs to observe mouse movement and clicks during this recording.</p>
        <div className="pointer-consent__actions">
          <button onClick={onSkip}>Record without zoom</button>
          <button className="pointer-consent__enable" autoFocus onClick={onEnable}>Enable zoom</button>
        </div>
      </div>
    </div>
  );
};

export default PointerCaptureConsent;
