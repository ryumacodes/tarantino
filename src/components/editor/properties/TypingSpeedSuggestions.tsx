import { typingSpeedSuggestions } from '../../../utils/timeline/typingSpeedSuggestions';
import { useTypingSpeedPreference } from '../../../hooks/useTypingSpeedPreference';
import { useShallow } from 'zustand/react/shallow';
import { useEditorStore } from '../../../stores/editor';

export function TypingSpeedSuggestions({disabled}: {disabled: boolean}) {
  const state = useEditorStore(useShallow(state => ({
    typingRanges: state.typingRanges, dismissedTypingRanges: state.dismissedTypingRanges,
    tracks: state.tracks, videoFilePath: state.videoFilePath,
    apply: state.speedUpTypingRange, applyAll: state.speedUpAllTypingRanges,
    dismiss: state.dismissTypingRange,
  })));
  const [rate,setRate] = useTypingSpeedPreference();
  const suggestions = typingSpeedSuggestions(state);
  if (!suggestions.length) return null;
  return <div className="section">
    <div className="section-header"><h3>Speed up typing</h3><p>Shorten keyboard-heavy sections without cutting their content.</p></div>
    <label className="control-group">Typing speed
      <div className="slider-control">
        <input className="editor-slider" type="range" min="1.5" max="8" step="0.1" value={rate}
          disabled={disabled} onChange={event => setRate(Number(event.target.value))} />
        <span className="value-display">{rate.toFixed(1)}×</span>
      </div>
    </label>
    <button className="speed-preset-btn" disabled={disabled || rate === 3} onClick={() => setRate(3)}>
      Reset speed
    </button>
    <button className="speed-preset-btn" disabled={disabled} onClick={() => state.applyAll(rate)}>
      Apply to all ({suggestions.length})
    </button>
    {suggestions.map(({clip,from,to,rangeStart}) => {
      const start=(clip.startTime+(from-clip.sourceIn)/clip.playbackRate)/1000;
      return <div key={`${clip.id}-${from}`}><button className="speed-preset-btn" disabled={disabled}
        onClick={() => state.apply(clip.id,from,to,rate)}>
        Speed up {start.toFixed(1)}–{(start+(to-from)/clip.playbackRate/1000).toFixed(1)}s
      </button>
      <button className="speed-preset-btn" disabled={disabled}
        aria-label={`Dismiss typing suggestion at ${start.toFixed(1)} seconds`}
        onClick={() => state.dismiss(rangeStart)}>Dismiss</button></div>;
    })}
    <p className="setting-hint">Each change can be undone.</p>
  </div>;
}
