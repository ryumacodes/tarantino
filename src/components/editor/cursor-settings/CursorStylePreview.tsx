import React from 'react';

export type CursorStyle = 'pointer' | 'circle' | 'filled' | 'outline' | 'dotted';

interface CursorStylePreviewProps {
  style: CursorStyle;
  size?: number;
}

export const CursorStylePreview: React.FC<CursorStylePreviewProps> = ({ style, size = 24 }) => {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" data-testid={`cursor-preview-${style}`}>
      {style === 'pointer' && (
        <path
          d="M5 3L5 19L9 15L12 21L14.5 20L11.5 14L17 14L5 3Z"
          fill="white"
          stroke="black"
          strokeWidth="1.5"
        />
      )}
      {style === 'circle' && (
        <circle cx="12" cy="12" r="8" fill="rgba(128, 128, 128, 0.8)" />
      )}
      {style === 'filled' && (
        <path
          d="M5 3L5 19L9 15L12 21L14.5 20L11.5 14L17 14L5 3Z"
          fill="black"
          stroke="white"
          strokeWidth="1.5"
        />
      )}
      {style === 'outline' && (
        <path
          d="M5 3L5 19L9 15L12 21L14.5 20L11.5 14L17 14L5 3Z"
          fill="none"
          stroke="white"
          strokeWidth="2"
        />
      )}
      {style === 'dotted' && (
        <path
          d="M5 3L5 19L9 15L12 21L14.5 20L11.5 14L17 14L5 3Z"
          fill="white"
          stroke="black"
          strokeWidth="1.5"
          strokeDasharray="2 2"
        />
      )}
    </svg>
  );
};

export default CursorStylePreview;
