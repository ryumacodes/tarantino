import { useEffect, type RefObject } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';

export function useCaptureSettingsWindow(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const dropdown = ref.current?.closest<HTMLElement>('.capture-bar__dropdown--settings');
    if (!dropdown) return;
    const content = dropdown.firstElementChild;
    if (!(content instanceof HTMLElement)) return;
    const win = getCurrentWindow();
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let original: LogicalSize | undefined;
    let pending = Promise.resolve();
    let lastHeight = 0;
    const resize = () => {
      if (disposed || !original) return;
      // Observe natural content, not the viewport-limited scroll container:
      // opening output options can increase scrollHeight without resizing it.
      const style = getComputedStyle(dropdown);
      const chrome = ['paddingTop', 'paddingBottom', 'borderTopWidth', 'borderBottomWidth']
        .reduce((sum, key) => sum + (parseFloat(style[key as keyof CSSStyleDeclaration] as string) || 0), 0);
      const desiredHeight = Math.ceil(content.getBoundingClientRect().height + chrome + 88);
      const height = Math.min(screen.availHeight - 100, desiredHeight);
      if (height === lastHeight) return;
      lastHeight = height;
      const size = new LogicalSize(original.width, Math.max(original.height, height));
      pending = pending.then(() => win.setSize(size)).catch(console.error);
    };
    void Promise.all([win.innerSize(), win.scaleFactor()]).then(([size, scale]) => {
      if (disposed) return;
      original = size.toLogical(scale);
      observer = new ResizeObserver(resize);
      observer.observe(content);
      resize();
    }).catch(console.error);
    return () => {
      disposed = true;
      observer?.disconnect();
      const size = original;
      if (size) void pending.then(() => win.setSize(size)).catch(console.error);
    };
  }, [ref]);
}
