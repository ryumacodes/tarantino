import { useEffect, useRef, useState } from 'react';
import { requestProjectPreview } from './projectPreviewQueue';
import { Film } from 'lucide-react';

export default function ProjectPreview({ path }: { path?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [frame, setFrame] = useState('');
  useEffect(() => {
    let disposed = false;
    setFrame('');
    if (!path || !ref.current) return;
    let cancel: (() => void) | undefined;
    let loaded = false;
    const observer = new IntersectionObserver(entries => {
      if (loaded) return;
      if (!entries.some(entry => entry.isIntersecting)) {
        cancel?.(); cancel = undefined;
        return;
      }
      if (cancel) return;
      cancel = requestProjectPreview(path, value => {
        if (disposed) return;
        loaded = true;
        setFrame(value);
        observer.disconnect();
      });
    }, { rootMargin: '80px' });
    observer.observe(ref.current);
    return () => { disposed = true; cancel?.(); observer.disconnect(); };
  }, [path]);
  return <div className="project-card-preview" ref={ref}>
    {frame ? <img src={frame} alt="" draggable={false} /> : <Film size={30} strokeWidth={1.3} aria-hidden="true" />}
  </div>;
}
