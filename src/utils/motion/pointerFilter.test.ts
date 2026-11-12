import { expect, it } from 'vitest';
import { removePointerShakes } from './pointerFilter';
import { DEFAULT_VISUAL_SETTINGS } from '../../stores/editor/constants';

it('keeps ordinary movement and drag releases at the new-project threshold', () => {
  const events = [
    { timestamp: 0, x: 100, y: 100, event_type: 'ButtonPress' },
    { timestamp: 16, x: 120, y: 110, event_type: 'Move' },
    { timestamp: 32, x: 140, y: 120, event_type: 'Move' },
    { timestamp: 48, x: 160, y: 130, event_type: 'ButtonRelease' },
  ];
  expect(removePointerShakes(events, DEFAULT_VISUAL_SETTINGS.shakesThreshold)).toEqual(events);
  expect(removePointerShakes(events, 3)).toEqual(events.slice(2, 3));
});

it('uses the native outlier and paired release rules without mutating input', () => {
  const events = [
    { timestamp: 0, x: 200, y: 200, event_type: 'ButtonPress' },
    { timestamp: 1, x: 0, y: 0, event_type: 'Move' },
    { timestamp: 2, x: 1, y: 1, event_type: 'Move' },
    { timestamp: 3, x: 2, y: 2, event_type: { ButtonRelease: {} } },
    { timestamp: 4, x: 100, y: 100, event_type: 'Move' },
    { timestamp: 5, x: 101, y: 101, event_type: 'Move' },
  ];
  const original = structuredClone(events);
  expect(removePointerShakes(events, 100).map(e => e.timestamp)).toEqual([1, 2, 4, 5]);
  expect(events).toEqual(original);
  expect(removePointerShakes(events, 0)).toBe(events);
  expect(removePointerShakes(events, NaN)).toBe(events);
});

it('uses strict pixel distance and sorts before comparing neighbors', () => {
  const events = [{ timestamp: 2, x: 6, y: 4 }, { timestamp: 0, x: 0, y: 0 },
    { timestamp: 1, x: 6, y: 4 }];
  expect(removePointerShakes(events, 10).map(e => e.timestamp)).toEqual([0, 1, 2]);
  expect(removePointerShakes(events, 9).map(e => e.timestamp)).toEqual([1, 2]);
});
