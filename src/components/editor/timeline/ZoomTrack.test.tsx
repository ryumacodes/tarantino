import { renderToStaticMarkup } from 'react-dom/server';
import type { ComponentProps } from 'react';
import { expect, it } from 'vitest';
import ZoomTrack from './ZoomTrack';

const props: ComponentProps<typeof ZoomTrack> = {
  pixelsPerMs: 0.1, zoomBlocks: [], zoomKeyframes: [], zoomAnalysis: null,
  previewZoomAnalysis: {
    indicators: [{ id: 'hint', click_time: 1000, click_x: 0.5, click_y: 0.5,
      preview_start: 500, preview_end: 2000, confidence: 0.5 }],
    total_clicks: 1, total_indicators: 1, session_duration: 3000, analysis_time_ms: 1,
  },
  zoomLoading: false, previewZoomLoading: false, selectedBlockId: null,
  isExporting: false, effectsEnabled: true, onToggleVisibility: () => {},
  onBlockClick: () => {}, onBlockDoubleClick: () => {}, onDragStart: () => {}, onResizeStart: () => {},
};
const emptyAnalysis = {
  zoom_blocks: [], total_clicks: 1, session_duration: 3000,
  config: { enabled: true, zoom_factor: 2, zoom_duration: 1000, min_click_spacing: 1000 },
};
it('shows provisional hints only while definitive analysis is unavailable', () => {
  const provisional = renderToStaticMarkup(<ZoomTrack {...props} />);
  expect(provisional).toContain('Preview zoom at 1000ms');
  const finished = renderToStaticMarkup(<ZoomTrack {...props} zoomAnalysis={emptyAnalysis} />);
  expect(finished).not.toContain('Preview zoom at');
  expect(finished).not.toContain('>Preview<');
  expect(finished).toContain('No zoom blocks in this project');
  expect(finished).toContain('class="track-count">0<');
});
it('does not hide completed edits behind late provisional analysis loading', () => {
  const finished = renderToStaticMarkup(<ZoomTrack {...props} zoomAnalysis={emptyAnalysis} previewZoomLoading />);
  expect(finished).not.toContain('Analyzing clicks');
  expect(finished).toContain('No zoom blocks in this project');
  const pending = renderToStaticMarkup(<ZoomTrack {...props} previewZoomLoading />);
  expect(pending).toContain('Analyzing clicks');
});
