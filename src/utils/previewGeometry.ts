interface Size { width: number; height: number }

export function contentInset(width: number, height: number, padding: number): number {
  const ratio = Math.max(0, Number.isFinite(padding) ? padding : 0) / 100;
  const shorter = Math.min(width, height);
  return Math.floor((shorter - shorter / (1 + 2 * ratio)) / 2);
}

/** Lay out the preview in output pixels first, then convert to scene units. */
export function previewGeometry({ output, source, viewport, padding, desktop }: {
  output: Size; source: Size; viewport: Size; padding: number;
  desktop?: { screen: Size; window: Size };
}) {
  const scale = Math.min(viewport.width / output.width, viewport.height / output.height);
  const inset = contentInset(output.width, output.height, padding);
  const contentWidth = output.width - 2 * inset;
  const contentHeight = output.height - 2 * inset;
  let width: number, height: number;
  if (desktop && desktop.screen.width > 0 && desktop.screen.height > 0
      && desktop.window.width > 0 && desktop.window.height > 0) {
    const desktopScale = Math.min(contentWidth / desktop.screen.width, contentHeight / desktop.screen.height);
    width = desktop.window.width * desktopScale;
    height = desktop.window.height * desktopScale;
  } else {
    const sourceAspect = source.width / source.height;
    const contentAspect = contentWidth / contentHeight;
    if (Math.abs(sourceAspect - contentAspect) < 0.01) {
      width = contentWidth;
      height = contentHeight;
    } else if (sourceAspect > contentAspect) {
      width = contentWidth;
      height = contentWidth / sourceAspect;
    } else {
      width = contentHeight * sourceAspect;
      height = contentHeight;
    }
  }
  width = Math.floor(Math.max(2, Math.min(contentWidth, Math.round(width))) / 2) * 2;
  height = Math.floor(Math.max(2, Math.min(contentHeight, Math.round(height))) / 2) * 2;
  return { canvasWidth: output.width * scale, canvasHeight: output.height * scale,
    planeWidth: width * scale, planeHeight: height * scale };
}
