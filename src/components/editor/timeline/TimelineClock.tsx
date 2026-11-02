import { useEditorStore } from '../../../stores/editor';
import { formatTime } from './utils';

export function TimelineTime({className}: {className: string}) {
  const time = useEditorStore(state => state.currentTime);
  return <span className={className}>{formatTime(time)}</span>;
}
export function CollapsedPlayhead({duration}: {duration: number}) {
  const time = useEditorStore(state => state.currentTime);
  return <div className="collapsed-playhead" style={{left:`${duration > 0 ? time/duration*100 : 0}%`}} />;
}
