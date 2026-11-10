import { zoomPresentationUpdate } from '../../../utils/motion/zoomPresentation';
import { useEditorStore } from '../../../stores/editor';
import type { ZoomBlock } from '../../../stores/editor/types';
import { manualZoomCenterUpdate } from '../../../utils/motion/zoomCenterBounds';

export function ZoomPresentationControls({ block, disabled }: { block: ZoomBlock; disabled: boolean }) {
  const updateZoomBlock = useEditorStore(state => state.updateZoomBlock);
  const captureMode = useEditorStore(state => state.captureMode);

  return <>
    <div className="control-group">
      <label>Zoom style</label>
      <div role="group" aria-label="Zoom style" style={{ display: 'flex', gap: '4px' }}>
        {(['screen', 'magnifier'] as const).map(style => <button key={style} type="button"
          aria-pressed={style === (block.loupe ? 'magnifier' : 'screen')}
          className={`editor-btn editor-btn--small ${style === (block.loupe ? 'magnifier' : 'screen')
            ? 'editor-btn--primary' : 'editor-btn--ghost'}`}
          disabled={disabled}
          onClick={() => {
            updateZoomBlock(block.id, {
              ...zoomPresentationUpdate(block, style),
              ...(style === 'screen' && manualZoomCenterUpdate(block, block.zoom_factor, captureMode)),
            });
          }}>{style === 'screen' ? 'Screen zoom' : 'Magnifier'}</button>)}
      </div>
      <small className="setting-hint">Magnifier enlarges a circular area while keeping the rest of the frame in place.</small>
    </div>
    {block.loupe && <div className="control-group">
      <label htmlFor="magnifier-radius">Magnifier size</label>
      <div className="slider-control">
        <input id="magnifier-radius" className="editor-slider" type="range"
          min="0.25" max="0.75" step="0.01" value={block.loupe.radius} disabled={disabled}
          onChange={event => updateZoomBlock(block.id, {
            loupe: { ...block.loupe!, radius: event.target.valueAsNumber },
          })} />
        <span className="value-display">{Math.round(block.loupe.radius * 100)}%</span>
      </div>
      <small className="setting-hint">Radius relative to the longest edge of the cropped recording.</small>
      <label htmlFor="magnifier-shadow">Shadow opacity</label>
      <div className="slider-control">
        <input id="magnifier-shadow" className="editor-slider" type="range"
          min="0" max="1" step="0.01" value={block.loupe.shadow_alpha} disabled={disabled}
          onChange={event => updateZoomBlock(block.id, {
            loupe: { ...block.loupe!, shadow_alpha: event.target.valueAsNumber },
          })} />
        <span className="value-display">{Math.round(block.loupe.shadow_alpha*100)}%</span>
      </div>
      <label className="checkbox-control">
        <input type="checkbox" checked={block.loupe.glass} disabled={disabled}
          onChange={event => updateZoomBlock(block.id, {
            loupe: { ...block.loupe!, glass: event.target.checked },
          })} />
        Glass effects
      </label>
      {block.loupe.glass && (['bevel', 'chromatic_aberration'] as const).map(property =>
        <div className="control-group" key={property}>
          <label htmlFor={`magnifier-${property}`}>{property === 'bevel' ? 'Curved edge' : 'Color separation'}</label>
          <div className="slider-control">
            <input id={`magnifier-${property}`} className="editor-slider" type="range"
              min="0" max="1" step="0.01" value={block.loupe![property]} disabled={disabled}
              onChange={event => updateZoomBlock(block.id, {
                loupe: { ...block.loupe!, [property]: event.target.valueAsNumber },
              })} />
            <span className="value-display">{Math.round(block.loupe![property]*100)}%</span>
          </div>
        </div>)}
    </div>}
  </>;
}
