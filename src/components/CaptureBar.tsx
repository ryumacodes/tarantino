import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Window } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { Menu, MenuItem } from '@tauri-apps/api/menu';
import DisplayPicker from './DisplayPicker';
import CaptureSettings, { CaptureConfig } from './CaptureSettings';
import PermissionStatus from './PermissionStatus';
import { 
  X, 
  Monitor, 
  Square, 
  Crop, 
  Smartphone,
  Camera,
  Mic,
  Volume2,
  Settings,
  Circle,
  ChevronDown
} from 'lucide-react';
import { useRecordingStore } from '../stores/recording';
import { cn } from '../utils/cn';

type CaptureMode = 'display' | 'window' | 'area' | 'device';
type InputDevice = { id: string; name: string };

const CaptureBar: React.FC = () => {
  const [captureMode, setCaptureMode] = useState<CaptureMode>('display');
  const [cameraEnabled, setCameraEnabled] = useState(false);
  const [micEnabled, setMicEnabled] = useState(false);
  const [systemAudioEnabled, setSystemAudioEnabled] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showDisplayPicker, setShowDisplayPicker] = useState(false);
  const [displays, setDisplays] = useState<any[]>([]);
  const [windows, setWindows] = useState<any[]>([]);
  const [devices, setDevices] = useState<any[]>([]);
  const [audioDevices, setAudioDevices] = useState<any>({ microphones: [], system_sources: [] });
  const [selectedDisplay, setSelectedDisplay] = useState<any>(null);
  const [selectedWindow, setSelectedWindow] = useState<string | null>(null);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);

  // Input device selection state
  const [selectedCameraDevice, setSelectedCameraDevice] = useState<string | null>(null);
  const [selectedMicDevice, setSelectedMicDevice] = useState<string | null>(null);
  const [selectedSystemAudioDevice, setSelectedSystemAudioDevice] = useState<string | null>(null);

  
  const [captureConfig, setCaptureConfig] = useState<CaptureConfig>({
    includeCursor: true,
    cursorSize: 'normal',
    highlightClicks: false,
    fps: 60,
    outputResolution: 'match',
    encoder: 'auto',
    container: 'mp4'
  });
  
  const { state, setRecordingState, stopRecording } = useRecordingStore();
  const isRecording = state === 'recording';

  useEffect(() => {
    loadDevices();
  }, []);

  useEffect(() => {
    // Only check for display changes when dropdowns are actually open
    if (!showDisplayPicker && !showSettings) {
      return; // Don't poll when UI is closed
    }

    // Listen for display configuration changes
    const checkDisplayChanges = async () => {
      try {
        const currentDisplays = await invoke<any[]>('get_displays');
        if (currentDisplays.length !== displays.length) {
          console.log('Display configuration changed, updating...');
          // Close any open dropdowns when displays change
          setShowDisplayPicker(false);
          setShowSettings(false);
          // Reload devices with new display config
          await loadDevices();
        }
      } catch (error) {
        console.error('Error checking display changes:', error);
      }
    };

    // Check for display changes periodically ONLY when dropdowns are open
    const interval = setInterval(checkDisplayChanges, 2000); // Reduced to every 2 seconds

    return () => clearInterval(interval);
  }, [showDisplayPicker, showSettings]); // Removed displays.length from dependencies

  useEffect(() => {
    // Listen for recording stopped event from backend (emits final media path as string)
    const unlisten = listen<string>('recording-stopped', async (event) => {
      console.log('Received recording-stopped event:', event.payload);

      // NOTE: Don't show capture bar here - let the editor open instead.
      // The capture bar will be shown when the editor is closed (via show_capture_bar command).

      if (event.payload) {
        await stopRecording(event.payload);
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [stopRecording]);

  const handleDragStart = async (e: React.MouseEvent) => {
    // Only start dragging if we clicked on the background, not on buttons or interactive elements
    const target = e.target as HTMLElement;
    if (target.tagName === 'BUTTON' || target.closest('button') || target.closest('.capture-bar__input')) {
      return;
    }
    
    console.log('Drag start clicked!', e);
    try {
      const window = Window.getCurrent();
      console.log('Got window:', window);
      await window.startDragging();
      console.log('Started dragging');
    } catch (error) {
      console.error('Failed to start dragging:', error);
    }
  };

  const loadDevices = async () => {
    try {
      const [displayList, windowList, deviceList, audioList] = await Promise.all([
        invoke<any[]>('get_displays'),
        invoke<any[]>('get_windows'),
        invoke<any[]>('get_devices'),
        invoke<any>('get_audio_devices')
      ]);
      
      console.log('Loaded devices:', { displayList, windowList, deviceList });
      
      setDisplays(displayList);
      setWindows(windowList);
      setDevices(deviceList);
      setAudioDevices(audioList);

      // Resolve selected display from backend state if available
      try {
        const selected = await invoke<any | null>('get_selected_display');
        if (selected) {
          setSelectedDisplay(selected);
        } else if (displayList.length > 0) {
          // Prefer primary display
          const primary = displayList.find((d: any) => d.is_primary) || displayList[0];
          setSelectedDisplay(primary);
          await invoke('capture_select_display', { id: primary.id });
        }
      } catch (e) {
        console.warn('Could not get selected display, falling back:', e);
        if (displayList.length > 0) {
          const primary = displayList.find((d: any) => d.is_primary) || displayList[0];
          setSelectedDisplay(primary);
          await invoke('capture_select_display', { id: primary.id });
        }
      }
    } catch (err) {
      console.error('Failed to load devices:', err);
    }
  };

  const handleModeChange = async (mode: CaptureMode) => {
    setCaptureMode(mode);
    // Backend expects: "desktop" | "window" | "device" (lowercase)
    const backendMode = mode === 'display' ? 'desktop' : mode;
    await invoke('capture_set_mode', { mode: backendMode });

    if (mode === 'display' && displays.length > 0) {
      await selectDisplay(displays[0].id);
    } else if (mode === 'window') {
      // Don't auto-select — user must pick a window from the menu
      if (!selectedWindow) {
        showWindowMenu();
      }
    } else if (mode === 'device' && devices.length > 0) {
      await selectDevice(devices[0].id);
    }
  };

  const selectDisplay = async (id: string) => {
    const display = displays.find(d => d.id === id);
    setSelectedDisplay(display);
    await invoke('capture_select_display', { id });
  };

  const selectWindow = async (id: string) => {
    setSelectedWindow(id);
    await invoke('capture_select_window', { id });
  };

  const selectDevice = async (id: string) => {
    setSelectedDevice(id);
    await invoke('capture_select_device', { id });
  };

  const handleCameraToggle = async () => {
    const enabled = !cameraEnabled;
    setCameraEnabled(enabled);
    await invoke('input_set_camera', {
      enabled,
      deviceId: selectedCameraDevice,
      shape: 'circle'
    });
  };

  const handleCameraDeviceSelect = async (deviceId: string) => {
    setSelectedCameraDevice(deviceId);
    if (cameraEnabled) {
      await invoke('input_set_camera', {
        enabled: true,
        deviceId,
        shape: 'circle'
      });
    }
  };

  const handleMicToggle = async () => {
    const enabled = !micEnabled;
    setMicEnabled(enabled);
    await invoke('input_set_mic', {
      enabled,
      deviceId: selectedMicDevice || audioDevices.microphones[0]?.id || null
    });
  };

  const handleMicDeviceSelect = async (deviceId: string) => {
    setSelectedMicDevice(deviceId);
    if (micEnabled) {
      await invoke('input_set_mic', {
        enabled: true,
        deviceId
      });
    }
  };

  const handleSystemAudioToggle = async () => {
    const enabled = !systemAudioEnabled;
    setSystemAudioEnabled(enabled);
    await invoke('input_set_system_audio', {
      enabled,
      sourceId: selectedSystemAudioDevice
    });
  };

  const handleSystemAudioDeviceSelect = async (sourceId: string) => {
    setSelectedSystemAudioDevice(sourceId);
    if (systemAudioEnabled) {
      await invoke('input_set_system_audio', {
        enabled: true,
        sourceId
      });
    }
  };


  const showCameraMenu = async () => {
    if (devices.length === 0) return;
    const items = await Promise.all(
      devices.map((device: any) =>
        MenuItem.new({
          text: `${selectedCameraDevice === device.id ? '✓ ' : '   '}${device.name || device.id}`,
          action: () => handleCameraDeviceSelect(device.id),
        })
      )
    );
    const menu = await Menu.new({ items });
    await menu.popup();
  };

  const showMicMenu = async () => {
    if (audioDevices.microphones.length === 0) return;
    const items = await Promise.all(
      audioDevices.microphones.map((mic: any) =>
        MenuItem.new({
          text: `${selectedMicDevice === mic.id ? '✓ ' : '   '}${mic.name || mic.id}`,
          action: () => handleMicDeviceSelect(mic.id),
        })
      )
    );
    const menu = await Menu.new({ items });
    await menu.popup();
  };

  const showSystemAudioMenu = async () => {
    const sources = audioDevices.system_sources || [];
    const items = await Promise.all(
      sources.length > 0
        ? sources.map((source: any) =>
            MenuItem.new({
              text: `${selectedSystemAudioDevice === source.id ? '✓ ' : '   '}${source.name || source.id}`,
              action: () => handleSystemAudioDeviceSelect(source.id),
            })
          )
        : [
            MenuItem.new({
              text: `${selectedSystemAudioDevice === 'default' ? '✓ ' : '   '}System Default`,
              action: () => handleSystemAudioDeviceSelect('default'),
            }),
          ]
    );
    const menu = await Menu.new({ items });
    await menu.popup();
  };

  const showWindowMenu = async () => {
    // Refresh window list before showing
    let freshWindows = windows;
    try {
      const windowList = await invoke<any[]>('get_windows');
      setWindows(windowList);
      freshWindows = windowList;
    } catch (e) {
      console.error('Failed to refresh windows:', e);
    }

    // Backend already filters to layer-0 app windows with titles.
    // Just exclude our own app here.
    const appWindows = freshWindows.filter((w: any) => {
      const appName = w.app_name || '';
      if (appName === 'Tarantino' || appName === 'tarantino') return false;
      return true;
    });

    if (appWindows.length === 0) {
      const noItems = await MenuItem.new({ text: 'No windows available', enabled: false });
      const menu = await Menu.new({ items: [noItems] });
      await menu.popup();
      return;
    }

    const items = await Promise.all(
      appWindows.map((w: any) =>
        MenuItem.new({
          text: `${selectedWindow === w.id ? '\u2713 ' : '   '}${w.app_name ? w.app_name + ' — ' : ''}${w.title}`,
          action: () => {
            setSelectedWindow(w.id);
            selectWindow(w.id);
          },
        })
      )
    );
    const menu = await Menu.new({ items });
    await menu.popup();
  };

  const getOutputResolution = () => {
    // For non-display modes (window, area), use 16:9 aspect ratio (Screen Studio style)
    // For display mode, use the display's actual dimensions
    const use16by9 = captureMode !== 'display';

    switch (captureConfig.outputResolution) {
      case '1080p': return { width: 1920, height: 1080 };
      case '1440p': return { width: 2560, height: 1440 };
      case '4k': return { width: 3840, height: 2160 };
      case 'match':
      default:
        if (use16by9 || !selectedDisplay) {
          // Default to 16:9 1080p for window/area modes
          return { width: 1920, height: 1080 };
        }
        // Use display's actual dimensions for display mode
        return {
          width: selectedDisplay.width,
          height: selectedDisplay.height
        };
    }
  };

  const canRecord = isRecording || captureMode !== 'window' || selectedWindow !== null;

  const handleRecord = async () => {
    if (!canRecord) return;
    if (isRecording) {
      // Show capture bar again when stopping recording
      const captureWindow = Window.getCurrent();
      await captureWindow.show();
      await captureWindow.setFocus();
      
      // Prefer instant stop to open editor immediately and background finalize
      const sessionPath = await invoke<string>('record_stop_instant_new');
      setRecordingState('review');
      // The backend will generate the sidecar file
    } else {
      const resolution = getOutputResolution();
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
      const path = `/tmp/${timestamp}.${captureConfig.container}`;
      
      // Use new recording API with native backends
      const targetType = captureMode === 'display' ? 'desktop' : (captureMode === 'window' ? 'window' : 'device');
      const targetId = targetType === 'desktop' ? (selectedDisplay?.id ?? '0') : (targetType === 'window' ? (selectedWindow ?? '0') : (selectedDevice ?? '0'));
      await invoke('record_start_new', {
        targetType,
        targetId,
        quality: captureConfig.outputResolution === 'match' ? 'High' : 'High',
        includeCursor: captureConfig.includeCursor,
        includeMicrophone: micEnabled,
        includeSystemAudio: systemAudioEnabled,
        outputPath: path
      });
      setRecordingState('recording');

      // Hide capture bar when recording starts
      const captureWindow = Window.getCurrent();
      await captureWindow.hide();
    }
  };

  const handleExit = async () => {
    if (isRecording) {
      await invoke('record_stop');
    }
    await invoke('exit');
  };

  return (
    <div className="capture-bar-pill" onMouseDown={handleDragStart}>
      <div
        className={cn('capture-bar__record', { recording: isRecording, disabled: !canRecord })}
        onClick={handleRecord}
      >
        <div className="record-dot" />
      </div>

      <div className="capture-bar__modes">
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'display' })}
          onClick={() => {
            handleModeChange('display');
            setShowDisplayPicker(!showDisplayPicker);
          }}
          onMouseEnter={async () => {
            console.log('Display hover - selectedDisplay:', selectedDisplay);
            if (selectedDisplay) {
              try {
                console.log('Calling show_display_preview with:', selectedDisplay.id);
                await invoke('show_display_preview', { displayId: selectedDisplay.id });
                console.log('show_display_preview completed successfully');
              } catch (error) {
                console.error('Failed to show display preview:', error);
              }
            } else {
              console.log('No selectedDisplay - not showing preview');
            }
          }}
          onMouseLeave={async () => {
            try {
              await invoke('hide_display_preview');
            } catch (error) {
              console.error('Failed to hide display preview:', error);
            }
          }}
          title="Capture Display"
        >
          <Monitor size={16} />
          <span>{selectedDisplay ? selectedDisplay.name : 'Display'}</span>
        </button>
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'window' })}
          onClick={() => {
            if (captureMode === 'window') {
              showWindowMenu();
            } else {
              handleModeChange('window');
            }
          }}
          title="Capture Window"
        >
          <Square size={16} />
          <span style={{ maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{captureMode === 'window' && selectedWindow ? (windows.find((w: any) => w.id === selectedWindow)?.title || 'Window') : 'Window'}</span>
          {captureMode === 'window' && <ChevronDown size={12} />}
        </button>
        {/* Area capture: considering but not decided — hiding for now
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'area' })}
          onClick={() => handleModeChange('area')}
          title="Capture Area"
        >
          <Crop size={16} />
          <span>Area</span>
        </button>
        */}
        {/* TODO: Device capture not yet implemented - iOS/Android mirroring
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'device' })}
          onClick={() => handleModeChange('device')}
          title="Capture Device"
        >
          <Smartphone size={18} />
          <span>Device</span>
        </button>
        */}
      </div>

      <div className="capture-bar__inputs">
        <div className="capture-bar__input">
          <button
            className={cn('capture-bar__input-toggle', { active: cameraEnabled })}
            onClick={handleCameraToggle}
            title={cameraEnabled ? 'Disable Camera' : 'Enable Camera'}
          >
            <Camera size={16} />
          </button>
          <button
            className="capture-bar__input-select"
            onClick={showCameraMenu}
            title="Select Camera"
          >
            <ChevronDown size={12} />
          </button>
        </div>

        <div className="capture-bar__input">
          <button
            className={cn('capture-bar__input-toggle', { active: micEnabled })}
            onClick={handleMicToggle}
            title={micEnabled ? 'Disable Microphone' : 'Enable Microphone'}
          >
            <Mic size={16} />
          </button>
          <button
            className="capture-bar__input-select"
            onClick={showMicMenu}
            title="Select Microphone"
          >
            <ChevronDown size={12} />
          </button>
        </div>

        <div className="capture-bar__input">
          <button
            className={cn('capture-bar__input-toggle', { active: systemAudioEnabled })}
            onClick={handleSystemAudioToggle}
            title={systemAudioEnabled ? 'Disable System Audio' : 'Enable System Audio'}
          >
            <Volume2 size={16} />
          </button>
          <button
            className="capture-bar__input-select"
            onClick={showSystemAudioMenu}
            title="Select System Audio"
          >
            <ChevronDown size={12} />
          </button>
        </div>
      </div>

      <button
        className="capture-bar__settings"
        onClick={() => setShowSettings(!showSettings)}
        title="Settings"
      >
        <Settings size={16} />
      </button>

      <button
        className="capture-bar__exit"
        onClick={handleExit}
        title="Exit"
      >
        <X size={16} />
      </button>
      
      {/* Dropdowns */}
      {showDisplayPicker && captureMode === 'display' && (
        <div className="capture-bar__dropdown">
          <DisplayPicker
            compact
            selectedId={selectedDisplay?.id}
            onSelect={(display) => {
              setSelectedDisplay(display);
              selectDisplay(display.id);
              setShowDisplayPicker(false);
            }}
          />
        </div>
      )}
      
      {showSettings && (
        <div className="capture-bar__dropdown capture-bar__dropdown--settings">
          <div className="space-y-4">
            <PermissionStatus 
              className="mb-4" 
              onPermissionsChanged={(status) => {
                // Could add logic here to disable recording if permissions aren't granted
                console.log('Permission status updated:', status);
              }}
            />
            
            <CaptureSettings
              compact
              config={captureConfig}
              onChange={setCaptureConfig}
              sourceResolution={selectedDisplay ? {
                width: selectedDisplay.width,
                height: selectedDisplay.height
              } : undefined}
            />
          </div>
        </div>
      )}
    </div>
  );
};

export default CaptureBar;