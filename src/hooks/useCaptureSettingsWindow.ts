import { useEffect, type RefObject } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { isLinuxRuntime } from '../utils/platform';

export function useCaptureSettingsWindow(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    if (!isLinuxRuntime) return;
    const dropdown = ref.current?.closest<HTMLElement>('.capture-bar__dropdown--settings');
    if (!dropdown) return;
    const win = getCurrentWindow();
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let original: LogicalSize | undefined;
    let pending = Promise.resolve();
    let lastHeight = 0;
    const resize = () => {
      if (disposed || !original) return;
      const height = Math.min(screen.availHeight - 100, Math.ceil(dropdown.scrollHeight + 88));
      if (height === lastHeight) return;
      lastHeight = height;
      const size = new LogicalSize(original.width, Math.max(original.height, height));
      pending = pending.then(() => win.setSize(size)).catch(console.error);
    };
    void Promise.all([win.innerSize(), win.scaleFactor()]).then(([size, scale]) => {
      if (disposed) return;
      original = size.toLogical(scale);
      observer = new ResizeObserver(resize);
      observer.observe(dropdown);
      resize();
    }).catch(console.error);
    return () => {
      disposed = true;
      observer?.disconnect();
      if (original) void pending.then(() => win.setSize(original!)).catch(console.error);
    };
  }, [ref]);
}
