// Timeline Types

export interface ProfessionalTimelineProps {
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  isExporting?: boolean;
}

export interface TrackVisibility {
  video: boolean;
  smartZoom: boolean;
  webcam: boolean;
  microphone: boolean;
  system: boolean;
}

export type DragType = 'playhead' | 'trim-start' | 'trim-end' | null;

export interface TimelineTrack {
  id: string;
  type: 'video' | 'audio' | 'webcam' | 'zoom';
  name: string;
  visible: boolean;
  locked: boolean;
}

export interface ZoomBlockPosition {
  id: string;
  left: number;
  width: number;
  startTime: number;
  endTime: number;
}
