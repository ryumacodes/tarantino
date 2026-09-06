import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { isLinuxRuntime } from '../utils/platform';
import { useCaptureSettingsWindow } from '../hooks/useCaptureSettingsWindow';

export default function VideoOutputLocation() {
  const ref = useRef<HTMLElement>(null);
  useCaptureSettingsWindow(ref);
  const [path, setPath] = useState<string>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<string>('get_video_output_directory')
      .then(value => { if (active) setPath(value); })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, []);

  return (
    <section ref={ref} data-linux={isLinuxRuntime || undefined} className="capture-bar__video-output" aria-label="Video output" onMouseDown={event => event.stopPropagation()}>
      <h3>Video output</h3>
      <p>Exported videos are saved to:</p>
      <div className="capture-bar__output-path" title={path} aria-live="polite">
        {path ?? (failed ? 'Unable to load save location' : 'Loading save location…')}
      </div>
    </section>
  );
}
