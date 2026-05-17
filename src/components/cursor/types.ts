// Cursor Overlay Types

export interface MouseEvent {
  timestamp: number;
  x: number;
  y: number;
  event_type: {
    Move?: undefined;
    ButtonPress?: { button: string };
    ButtonRelease?: { button: string };
    Wheel?: { delta_x: number; delta_y: number };
  };
  display_id?: string;
}

export interface Ripple {
  id: number;
  x: number;
  y: number;
  startTime: number;
  duration: number;
}

export interface CircleHighlight {
  id: number;
  x: number;
  y: number;
  startTime: number;
  duration: number;
}

export interface SpringState {
  value: number;
  velocity: number;
}

export interface SpringConfig {
  tension: number;
  friction: number;
  mass: number;
}

export type CursorStyle = 'pointer' | 'circle' | 'filled' | 'outline' | 'dotted';

export interface VideoTransform {
  scale: number;
  offsetX: number;
  offsetY: number;
  viewportWidth: number;
  viewportHeight: number;
  planeWidth: number;
  planeHeight: number;
}

export interface MouseCursorOverlayProps {
  videoRef: React.RefObject<HTMLVideoElement | null>;
  sidecarPath?: string;
  videoWidth?: number;
  videoHeight?: number;
  visible?: boolean;
  videoDuration?: number;
  videoTransform?: VideoTransform;
}

export interface TrailPosition {
  x: number;
  y: number;
  time: number;
}

export interface CursorSpringState {
  x: SpringState;
  y: SpringState;
}
