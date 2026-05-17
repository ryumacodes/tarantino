export {};

declare global {
  interface Window {
    __TARANTINO_VIDEO_ELEMENT?: HTMLVideoElement;
    __TARANTINO_SEEK_VIDEO?: (timeMs: number) => void;
    __TARANTINO_SET_PLAYING?: (playing: boolean) => void;
    __TARANTINO_CURRENT_TIME?: number;
    __TARANTINO_WAS_PLAYING?: boolean;
  }
}
