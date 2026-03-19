// Timeline Utility Functions

/**
 * Format time in ms to MM:SS.mmm format
 */
export const formatTime = (ms: number): string => {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const milliseconds = Math.floor((ms % 1000) / 10); // Show 2 decimal places
  return `${minutes}:${seconds.toString().padStart(2, '0')}.${milliseconds.toString().padStart(2, '0')}`;
};

/**
 * Format time in ms to MM:SS format (no milliseconds)
 */
export const formatTimeShort = (ms: number): string => {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
};

/**
 * Calculate the position percentage for a time value within a duration
 */
export const calculatePositionPercent = (time: number, duration: number): number => {
  if (duration <= 0) return 0;
  return Math.max(0, Math.min(100, (time / duration) * 100));
};

/**
 * Calculate time from a position percentage and duration
 */
export const calculateTimeFromPercent = (percent: number, duration: number): number => {
  return Math.max(0, Math.min(duration, (percent / 100) * duration));
};

/**
 * Get smart default track visibility based on available data
 */
export const getSmartTrackVisibility = () => ({
  video: true,
  smartZoom: false, // Will be set dynamically based on data
  webcam: true,
  microphone: true,
  system: true,
});

/**
 * Clamp a value between min and max
 */
export const clamp = (value: number, min: number, max: number): number => {
  return Math.max(min, Math.min(max, value));
};
