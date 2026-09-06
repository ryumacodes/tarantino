import { invoke } from '@tauri-apps/api/core';
import type { CaptureConfig } from './CaptureSettings';
import type React from 'react';
import { Window } from '@tauri-apps/api/window';
import { Menu, MenuItem } from '@tauri-apps/api/menu';

export type CaptureMode = 'display' | 'window' | 'area' | 'device';
export type WebcamShape = 'circle' | 'roundrect';
interface NativeMenuChoice {
  id: string;
  name?: string;
}
export interface CameraDevice extends NativeMenuChoice {
  name: string;
  is_default?: boolean;
}
interface CaptureWindowInfo {
  id: string;
  title?: string;
  app_name?: string;
}
export const isRecordableWindow = (windowInfo: CaptureWindowInfo) => {
  const appName = (windowInfo.app_name || '').toLowerCase();
  const title = (windowInfo.title || '').toLowerCase();
  return appName !== 'tarantino' && title !== 'tarantino' && !title.includes('web inspector');
};
export const createSelectionItems = <T extends NativeMenuChoice>(
  choices: T[],
  selectedId: string | null,
  onSelect: (choice: T) => void,
  label: (choice: T) => string = (choice) => choice.name || choice.id,
) => Promise.all(
  choices.map((choice) => MenuItem.new({
    text: `${selectedId === choice.id ? '✓ ' : '   '}${label(choice)}`,
    action: () => onSelect(choice),
  })),
);
export const popupNativeMenu = async (items: Awaited<ReturnType<typeof MenuItem.new>>[]) => {
  const menu = await Menu.new({ items });
  await menu.popup();
};
export const handleCaptureBarDrag = async (event: React.MouseEvent) => {
  const target = event.target as HTMLElement;
  if (
    target.tagName === 'BUTTON'
    || target.closest('button')
    || target.closest('.capture-bar__record')
    || target.closest('.capture-bar__input')
  ) return;
  try {
    await Window.getCurrent().startDragging();
  } catch (error) {
    console.error('Failed to start dragging:', error);
  }
};


export const initialCaptureConfig: CaptureConfig = {
  includeCursor: true,
  cursorSize: 'normal',
  highlightClicks: false,
  fps: 60,
  outputResolution: 'match',
  encoder: 'auto',
  container: 'mp4',
};

export const exitCaptureBar = async (isRecording: boolean) => {
  if (isRecording) {
    await invoke('record_stop');
  }
  await invoke('exit');
};
