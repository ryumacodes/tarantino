import React from 'react';

interface PlayheadProps {
  currentTime: number;
  pixelsPerMs: number;
  trackHeaderWidth: number;
  onMouseDown: (e: React.MouseEvent) => void;
}

const Playhead: React.FC<PlayheadProps> = ({
  currentTime,
  pixelsPerMs,
  trackHeaderWidth,
  onMouseDown,
}) => {
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
