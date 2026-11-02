import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Keyboard, X } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useEditorStore } from '../../../stores/editor';
import { typingSpeedSuggestions } from '../../../utils/timeline/typingSpeedSuggestions';
import { useTypingSpeedPreference } from '../../../hooks/useTypingSpeedPreference';
import './typing-markers.css';

type Props = { pixelsPerMs: number; viewportLeft: number; viewportRight: number;
  trimStart: number; trimEnd: number; isExporting: boolean };

function TypingMarkers(props: Props) {
  const state = useEditorStore(useShallow(state => ({
    typingRanges: state.typingRanges, dismissedTypingRanges: state.dismissedTypingRanges,
    tracks: state.tracks, videoFilePath: state.videoFilePath,
    apply: state.speedUpTypingRange, applyAll: state.speedUpAllTypingRanges,
    dismiss: state.dismissTypingRange, seek: state.setCurrentTime, setPlaying: state.setIsPlaying,
  })));
  const suggestions = useMemo(() => typingSpeedSuggestions(state),
    [state.typingRanges,state.dismissedTypingRanges,state.tracks,state.videoFilePath]);
  const [selected, setSelected] = useState<string | null>(null);
  const [rate, setRate] = useTypingSpeedPreference();
  const closeButton = useRef<HTMLButtonElement>(null);
  const opener = useRef<HTMLButtonElement | null>(null);
  const key = (item: typeof suggestions[number]) => `${item.clip.id}:${item.rangeStart}`;
  const active = suggestions.find(item => key(item) === selected);
  const close = () => { setSelected(null); opener.current?.focus(); };
  useEffect(() => {
    if (!active || props.isExporting) return;
    closeButton.current?.focus();
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); }
    };
    window.addEventListener('keydown', escape, true);
    return () => window.removeEventListener('keydown', escape, true);
  }, [selected, !!active, props.isExporting]);

  return <>
    {suggestions.map(item => {
      const start = item.clip.startTime+(item.from-item.clip.sourceIn)/item.clip.playbackRate;
      const end = start+(item.to-item.from)/item.clip.playbackRate;
      const left = Math.max(start,props.trimStart)*props.pixelsPerMs;
      const right = Math.min(end,props.trimEnd)*props.pixelsPerMs;
      if (right <= left || right < props.viewportLeft || left > props.viewportRight) return null;
      return <button key={key(item)} className="typing-marker" disabled={props.isExporting}
        style={{left, width: Math.max(2,right-left)}}
        title={`Typing ${(start/1000).toFixed(1)}–${(end/1000).toFixed(1)}s`}
        aria-label={`Edit typing section at ${(start/1000).toFixed(1)} seconds`}
        aria-haspopup="dialog" aria-expanded={selected === key(item)}
        onMouseDown={event => event.stopPropagation()}
        onClick={event => { event.stopPropagation(); opener.current=event.currentTarget; setSelected(key(item)); }}>
        <Keyboard size={13} /><span>Typing</span>
      </button>;
    })}
    {active && !props.isExporting && createPortal(
      <div className="typing-popover-backdrop" onMouseDown={event => event.stopPropagation()}
        onClick={event => { event.stopPropagation(); if(event.target === event.currentTarget) close(); }}>
        <section className="typing-popover" role="dialog" aria-modal="true" aria-label="Speed up typing"
          onKeyDown={event => {
            event.stopPropagation();
            if(event.key !== 'Tab') return;
            const controls=Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled)'));
            const first=controls[0], last=controls[controls.length-1];
            if(event.shiftKey && document.activeElement === first) {event.preventDefault();last?.focus();}
            else if(!event.shiftKey && document.activeElement === last) {event.preventDefault();first?.focus();}
          }}>
          <header><h3>Speed up typing</h3><button ref={closeButton} aria-label="Close typing controls" onClick={close}><X size={16}/></button></header>
          <p>Keep the content and shorten the typing.</p>
          <label>Typing speed <output>{rate.toFixed(1)}×</output>
            <input type="range" min="1.5" max="8" step="0.1" value={rate} onChange={event => setRate(Number(event.target.value))}/>
          </label>
          <div className="typing-popover-actions">
            <button onClick={() => {state.setPlaying(false);state.seek(Math.max(props.trimStart,active.clip.startTime+(active.from-active.clip.sourceIn)/active.clip.playbackRate));close();}}>Preview section</button>
            <button onClick={() => setRate(3)} disabled={rate===3}>Reset speed</button>
            <button onClick={() => {state.apply(active.clip.id,active.from,active.to,rate);close();}}>Apply suggestion</button>
            <button onClick={() => {state.applyAll(rate);close();}}>Apply all ({suggestions.length})</button>
            <button onClick={() => {state.dismiss(active.rangeStart);close();}}>Dismiss suggestion</button>
          </div>
        </section>
      </div>,document.body)}
  </>;
}

export default memo(TypingMarkers);
