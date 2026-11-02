import { useEditorStore } from '../../../stores/editor';
import React from 'react';

interface PlayheadProps {
  pixelsPerMs: number;
  trackHeaderWidth: number;
  onMouseDown: (e: React.MouseEvent) => void;
}

const Playhead: React.FC<PlayheadProps> = ({
  pixelsPerMs,
  trackHeaderWidth,
  onMouseDown,
}) => {
  const currentTime = useEditorStore(state => state.currentTime);
  return (
    <div
      className="timeline-playhead"
      style={{ left: `${trackHeaderWidth + currentTime * pixelsPerMs}px` }}
      onMouseDown={onMouseDown}
    >
      <div className="playhead-line" />
      <div className="playhead-handle" />
    </div>
  );
};

export default Playhead;
