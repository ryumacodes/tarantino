// Editor Store Constants

import type { ExportSettings, VisualSettings } from './types';

export const DEFAULT_EXPORT_SETTINGS: ExportSettings = {
  resolution: '1080p',
  frameRate: 60,
  quality: 'high',
  format: 'mp4',
};

// Resolution dimensions map
export const RESOLUTION_DIMENSIONS = {
  '720p': { width: 1280, height: 720 },
  '1080p': { width: 1920, height: 1080 },
  '1440p': { width: 2560, height: 1440 },
  '4k': { width: 3840, height: 2160 },
} as const;

// Aspect ratio presets with dimensions
export const ASPECT_RATIOS = {
  '16:9': { width: 16, height: 9, label: 'Landscape', description: 'YouTube, presentations' },
  '9:16': { width: 9, height: 16, label: 'Portrait', description: 'TikTok, Reels, Stories' },
  '4:3': { width: 4, height: 3, label: 'Classic', description: 'Traditional video' },
  '1:1': { width: 1, height: 1, label: 'Square', description: 'Instagram, social' },
  '21:9': { width: 21, height: 9, label: 'Ultra-wide', description: 'Cinematic' },
  'auto': { width: 0, height: 0, label: 'Original', description: 'Keep source aspect' },
} as const;

// Device frame specifications (dimensions are for the screen area within the frame)
export const DEVICE_FRAMES = {
  'none': {
    label: 'None',
    description: 'No device frame',
    screenAspect: null,
    bezel: 0,
  },
  'iphone-15-pro': {
    label: 'iPhone 15 Pro',
    description: 'Dynamic Island, titanium',
    screenAspect: 19.5 / 9,
    bezel: 20, // px around screen
    cornerRadius: 55,
    notch: 'dynamic-island',
  },
  'iphone-15': {
    label: 'iPhone 15',
    description: 'Standard iPhone',
    screenAspect: 19.5 / 9,
    bezel: 20,
    cornerRadius: 50,
    notch: 'dynamic-island',
  },
  'ipad-pro': {
    label: 'iPad Pro',
    description: '12.9" display',
    screenAspect: 4 / 3,
    bezel: 30,
    cornerRadius: 25,
    notch: null,
  },
  'macbook-pro': {
    label: 'MacBook Pro',
    description: 'Laptop mockup',
    screenAspect: 16 / 10,
    bezel: 15,
    cornerRadius: 10,
    notch: 'camera',
  },
  'browser': {
    label: 'Browser',
    description: 'Chrome-style window',
    screenAspect: null, // Flexible
    bezel: 40, // Top bar height
    cornerRadius: 10,
    notch: null,
  },
} as const;

// Predefined wallpapers (Screen Studio style)
export const WALLPAPERS = {
  'gradient-purple': { type: 'gradient', colors: ['#667eea', '#764ba2'] },
  'gradient-blue': { type: 'gradient', colors: ['#2193b0', '#6dd5ed'] },
  'gradient-sunset': { type: 'gradient', colors: ['#ff6b6b', '#feca57', '#ff9ff3'] },
  'gradient-ocean': { type: 'gradient', colors: ['#0f0c29', '#302b63', '#24243e'] },
  'gradient-mint': { type: 'gradient', colors: ['#11998e', '#38ef7d'] },
  'gradient-peach': { type: 'gradient', colors: ['#ee9ca7', '#ffdde1'] },
  'solid-dark': { type: 'solid', color: '#1a1a2e' },
  'solid-light': { type: 'solid', color: '#f5f5f5' },
  'solid-blue': { type: 'solid', color: '#0a192f' },
} as const;

// Spring physics presets (Screen Studio style: Slow, Mellow, Quick, Rapid)
// Critically-damped or slightly over-damped to prevent bounce/overshoot
// Critical damping: friction = 2 * sqrt(tension * mass)
export const SPRING_PRESETS = {
  slow:   { tension: 120, friction: 28, mass: 1.0 },   // Gentle, smooth glide
  mellow: { tension: 170, friction: 30, mass: 1.0 },   // Balanced, no overshoot
  quick:  { tension: 280, friction: 38, mass: 1.0 },   // Snappy, precise landing
  rapid:  { tension: 400, friction: 44, mass: 1.0 },   // Fast, decisive stop
} as const;

export type SpringPreset = keyof typeof SPRING_PRESETS;

// Legacy preset format (for backward compatibility during transition)
export const ZOOM_SPEED_PRESETS = {
  slow: { zoomIn: 400, zoomOut: 400, easing: 'ease-out' },
  mellow: { zoomIn: 300, zoomOut: 300, easing: 'ease-in-out' },
  quick: { zoomIn: 200, zoomOut: 200, easing: 'ease-in-out' },
  rapid: { zoomIn: 150, zoomOut: 150, easing: 'ease-in' },
} as const;

export const DEFAULT_VISUAL_SETTINGS: VisualSettings = {
  // Defaults show raw recording without styling - users can add effects later
  backgroundType: 'solid',
  backgroundColor: '#000000',
  gradientStops: [
    { color: '#667eea', position: 0 },
    { color: '#764ba2', position: 100 },
  ],
  gradientDirection: 'to-bottom-right',
  wallpaperId: null,
  customBackgroundImage: null,
  padding: 0,
  cornerRadius: 0,
  shadowEnabled: false,
  shadowIntensity: 40,
  shadowBlur: 60,
  shadowOffsetX: 0,
  shadowOffsetY: 10,
  inset: 0,
  cursorScale: 3.0,
  cursorSmoothing: 0.15,
  hideCursorWhenIdle: false,
  idleTimeout: 2000,
  // Cursor customization defaults
  cursorColor: '#ffffff',
  cursorHighlightColor: '#ff6b6b',
  cursorRippleColor: '#64b4ff',
  cursorShadowIntensity: 30,
  cursorTrailEnabled: false,
  cursorTrailLength: 15,
  cursorTrailOpacity: 0.5,
  // Cursor style & behavior
  cursorStyle: 'pointer',
  alwaysUsePointer: false,
  hideCursor: false,
  loopCursorPosition: false,
  // Click effects
  clickEffect: 'ripple',
  // Cursor rotation
  cursorRotation: 0,
  rotateCursorWhileMoving: false,
  rotationIntensity: 30,
  // Advanced cursor options
  stopCursorAtEnd: false,
  stopCursorDuration: 500,
  removeCursorShakes: true,
  shakesThreshold: 3,
  optimizeCursorChanges: true,
  // Animation
  zoomSpeedPreset: 'mellow',
  cursorSpeedPreset: 'mellow',
  motionBlurEnabled: true,
  motionBlurPanIntensity: 20, // Subtle blur during camera pans
  motionBlurZoomIntensity: 0, // Clean zooms by default (Screen Studio style)
  motionBlurCursorIntensity: 0, // No cursor blur by default
  aspectRatio: 'auto',
  deviceFrame: 'none',
  deviceFrameColor: 'black',
};
