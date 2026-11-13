import { useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useEditorStore } from '../../../stores/editor';
import { normalizedCrop, resolveSourceCrop } from '../../../utils/sourceCrop';

function CropField({ label, value, min, max, disabled, onCommit }: {
  label: string; value: number; min: number; max: number; disabled: boolean; onCommit: (value: number) => void;
}) {
  const display = String(Number(value.toFixed(3)));
  const [draft, setDraft] = useState(display);
  useEffect(() => setDraft(display), [display]);
  return <label>{label}<input aria-label={`Crop ${label.toLowerCase()}`} type="number"
    min={min} max={max} step="1" value={draft} disabled={disabled}
    onChange={event => setDraft(event.target.value)}
    onBlur={() => {
      const next = Number(draft);
      if (draft !== display && draft.trim() && Number.isFinite(next)) onCommit(next);
      setDraft(display);
    }}
    onKeyDown={event => {
      if (event.key === 'Enter') { event.stopPropagation(); event.currentTarget.blur(); }
    }} /></label>;
}

export function SourceCropControls({ disabled }: { disabled: boolean }) {
  const { videoWidth,videoHeight,visualSettings,updateVisualSettings } = useEditorStore(useShallow(state => ({
    videoWidth: state.videoWidth, videoHeight: state.videoHeight,
    visualSettings: state.visualSettings, updateVisualSettings: state.updateVisualSettings,
  })));
  if (!videoWidth || !videoHeight) return null;
  const crop = resolveSourceCrop(visualSettings.sourceCrop ?? {x:0,y:0,width:1,height:1},videoWidth,videoHeight);
  const change = (key: 'x'|'y'|'width'|'height', value: number) => {
    if (!Number.isFinite(value)) return;
    const next = {...crop,[key]:Math.round(value)};
    next.x=Math.max(0,Math.min(videoWidth-1,next.x));
    next.y=Math.max(0,Math.min(videoHeight-1,next.y));
    next.width=Math.max(1,Math.min(videoWidth-next.x,next.width));
    next.height=Math.max(1,Math.min(videoHeight-next.y,next.height));
    updateVisualSettings({sourceCrop:normalizedCrop(next)});
  };
  return <section className="section">
    <div className="section-header"><h3>Source crop</h3><p>Choose the part of the recording to show.</p></div>
    <div className="source-crop-fields">
      {([['x','Left'],['y','Top'],['width','Width'],['height','Height']] as const).map(([key,label]) =>
        <CropField key={key} label={label}
          min={key==='x'||key==='y'?0:1}
          max={key==='x'?videoWidth-1:key==='y'?videoHeight-1:key==='width'?videoWidth-crop.x:videoHeight-crop.y}
          value={crop[key]} disabled={disabled} onCommit={value => change(key,value)} />)}
    </div>
    <button className="speed-preset-btn" disabled={disabled || !visualSettings.sourceCrop}
      onClick={() => updateVisualSettings({sourceCrop:undefined})}>Reset crop</button>
  </section>;
}
