import { beforeEach, describe, expect, it } from 'vitest';
import { useRecordingStore } from './recording';

beforeEach(() => useRecordingStore.setState({ state: 'idle', recordingError: null, startTime: null }));

describe('recording failures', () => {
  it('does not overwrite a failure that arrives before the start command resolves', () => {
    const store = useRecordingStore.getState();
    store.setRecordingState('prerecord');
    store.failRecording('PipeWire disconnected');
    expect(store.confirmRecordingStarted()).toBe(false);
    expect(useRecordingStore.getState().state).toBe('idle');
    expect(useRecordingStore.getState().recordingError).toBe('PipeWire disconnected');
  });

  it('stops an active recording and allows a fresh take after failure', () => {
    const store = useRecordingStore.getState();
    store.setRecordingState('prerecord');
    expect(store.confirmRecordingStarted()).toBe(true);
    store.failRecording('Encoder failed');
    expect(useRecordingStore.getState().state).toBe('idle');
    store.setRecordingState('prerecord');
    expect(useRecordingStore.getState().recordingError).toBeNull();
    expect(store.confirmRecordingStarted()).toBe(true);
  });
});
