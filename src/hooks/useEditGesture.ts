import { useCallback, useEffect, useRef } from 'react';
import { useEditorStore } from '../stores/editor';

/** Begin before the first mutation, close on release or component removal. */
export function useEditGesture() {
  const active = useRef(false);
  const begin = useCallback(() => {
    if (active.current) return;
    active.current = true;
    useEditorStore.getState().beginHistoryGroup();
  }, []);
  const end = useCallback(() => {
    if (!active.current) return;
    active.current = false;
    useEditorStore.getState().endHistoryGroup();
  }, []);
  useEffect(() => end, [end]);
  return { begin, end };
}
