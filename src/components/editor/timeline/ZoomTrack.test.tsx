import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it, vi } from 'vitest';
import type { ComponentProps } from 'react';
import ZoomTrack from './ZoomTrack';
import { projectSourceRange, zoomResizeAvailability } from '../../../utils/timeline/sourceRanges';
import type { TimelineClip, TimelineTrack } from '../../../stores/editor/types';

const track: TimelineTrack = { id: 'video', name: 'Video', type: 'video', clips: [],
  height: 60, visible: true, muted: false, solo: false, locked: false, order: 0 };
const clip = (id: string, startTime: number, sourceIn: number, sourceOut: number, playbackRate: number): TimelineClip => ({
  id, name: id, type: 'video', trackId: track.id, startTime, sourceIn, sourceOut, playbackRate,
  duration: (sourceOut - sourceIn) / playbackRate, enabled: true, locked: false,
});
const props: ComponentProps<typeof ZoomTrack> = {
  pixelsPerMs: 0.1, zoomBlocks: [], zoomKeyframes: [], previewZoomAnalysis: null,
  zoomLoading: false, previewZoomLoading: false, zoomAnalysis: null,
  selectedBlockId: null, isExporting: false, effectsEnabled: true,
  onToggleVisibility: vi.fn(), onBlockClick: vi.fn(), onBlockDoubleClick: vi.fn(),
  onDragStart: vi.fn(), onResizeStart: vi.fn(),
};

it('shows resize handles only at zoom endpoints, not internal cut or speed boundaries', () => {
  const state = { duration: 6000, tracks: [track], clips: [
    clip('first', 0, 0, 2000, 1), clip('fast', 2000, 3000, 6000, 2),
  ] };
  const views = projectSourceRange(state, 1000, 5000);
  expect(views).toHaveLength(2);
  const markup = views.map(view => renderToStaticMarkup(<ZoomTrack {...props} zoomBlocks={[
    { ...view, id: 'zoom', zoomFactor: 2, isManual: true,
    canResize: zoomResizeAvailability(view, {start:0,end:6000}) },
  ]}/>));
  expect(markup[0]).toContain('resize-handle-left');
  expect(markup[0]).not.toContain('resize-handle-right');
  expect(markup[1]).not.toContain('resize-handle-left');
  expect(markup[1]).toContain('resize-handle-right');
});

it('does not offer a resize handle for an endpoint removed by trimming', () => {
  const state = { duration: 6000, tracks: [track], clips: [clip('trimmed', 0, 2000, 4000, 1)] };
  const zoomBlocks = projectSourceRange(state, 1000, 5000).map(view => ({
    ...view, id: 'zoom', zoomFactor: 2, isManual: true,
    canResize: zoomResizeAvailability(view, {start:0,end:6000}),
  }));
  const markup = renderToStaticMarkup(<ZoomTrack {...props} zoomBlocks={zoomBlocks}/>);
  expect(markup).toContain('zoom-block-container');
  expect(markup).not.toContain('resize-handle');
});
