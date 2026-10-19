import { useEffect, type RefObject } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalSize } from '@tauri-apps/api/dpi';
import { isLinuxRuntime } from '../utils/platform';

// GTK's window scale can differ from WebKit's CSS pixel scale.
export function useLinuxCaptureWindow(ref: RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    if (!isLinuxRuntime || !ref.current) return;
    const root = ref.current;
    const win = getCurrentWindow();
    let disposed = false;
    let frame = 0;
    let running = false;
    let dirty = false;

    const fit = async () => {
      if (disposed) return;
      if (running) { dirty = true; return; }
      // The settings panel temporarily owns the expanded window geometry.
      if (root.querySelector('.capture-bar__dropdown--settings')) return;
      running = true;
      try {
        const native = await win.innerSize();
        if (disposed || root.querySelector('.capture-bar__dropdown--settings')) return;
        const viewportWidth = window.innerWidth;
        const viewportHeight = window.innerHeight;
        if (!viewportWidth || !viewportHeight) return;
        const bar = root.querySelector<HTMLElement>('.capture-bar-pill');
        if (!bar) return;
        const bounds = bar.getBoundingClientRect();
        const width = Math.ceil(Math.max(root.offsetWidth, bounds.width + 10));
        const height = Math.ceil(Math.max(root.offsetHeight, bounds.bottom));
        if (Math.abs(viewportWidth - width) <= 1 && Math.abs(viewportHeight - height) <= 1) return;
        await win.setSize(new PhysicalSize(
          Math.ceil(width * native.width / viewportWidth),
          Math.ceil(height * native.height / viewportHeight),
        ));
      } catch (error) {
        console.error('Unable to fit the Linux capture window:', error);
      } finally {
        running = false;
        if (dirty && !disposed) { dirty = false; schedule(); }
      }
    };
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => { void fit(); });
    };
    const observer = new ResizeObserver(schedule);
    observer.observe(root);
    const bar = root.querySelector('.capture-bar-pill');
    if (bar) observer.observe(bar);
    window.addEventListener('resize', schedule);
    void document.fonts.ready.then(() => { if (!disposed) schedule(); });
    schedule();
    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener('resize', schedule);
    };
  }, [ref]);
}
