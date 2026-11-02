import { useEffect } from 'react';
import { useEditorStore } from '../stores/editor';

/** Each range-control pointer drag or held navigation key is one edit. */
export function useHistoryGestures(disabled: boolean) {
  useEffect(() => {
    if (disabled) return;
    let active: 'pointer' | 'keyboard' | null = null;
    let activeTarget: EventTarget | null = null;
    const store = useEditorStore.getState();
    const finish = () => {
      if (active === null) return;
      active = null; activeTarget = null;
      store.endHistoryGroup();
    };
    const isRange = (target: EventTarget | null) => target instanceof HTMLInputElement && target.type === 'range';
    const start = (kind: 'pointer' | 'keyboard', target: EventTarget | null) => {
      if (active === kind && activeTarget === target) return;
      finish(); active = kind; activeTarget = target; store.beginHistoryGroup();
    };
    const pointerDown = (event: PointerEvent) => {
      if (event.button === 0 && isRange(event.target)) start('pointer', event.target);
    };
    const pointerUp = () => { if (active === 'pointer') finish(); };
    const keys = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'];
    const keyDown = (event: KeyboardEvent) => {
      if (!event.altKey && !event.metaKey && !event.ctrlKey && isRange(event.target) && keys.includes(event.key)) start('keyboard', event.target);
    };
    const keyUp = (event: KeyboardEvent) => { if (active === 'keyboard' && keys.includes(event.key)) finish(); };
    const focusOut = (event: FocusEvent) => { if (event.target === activeTarget) finish(); };
    document.addEventListener('pointerdown', pointerDown, true);
    document.addEventListener('pointerup', pointerUp, true);
    document.addEventListener('pointercancel', pointerUp, true);
    document.addEventListener('keydown', keyDown, true);
    document.addEventListener('keyup', keyUp, true);
    document.addEventListener('focusout', focusOut, true);
    window.addEventListener('blur', finish);
    return () => {
      finish();
      document.removeEventListener('pointerdown', pointerDown, true);
      document.removeEventListener('pointerup', pointerUp, true);
      document.removeEventListener('pointercancel', pointerUp, true);
      document.removeEventListener('keydown', keyDown, true);
      document.removeEventListener('keyup', keyUp, true);
      document.removeEventListener('focusout', focusOut, true);
      window.removeEventListener('blur', finish);
    };
  }, [disabled]);
}
