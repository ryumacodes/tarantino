import React, { useState, useEffect, useRef } from 'react'; import { invoke } from '@tauri-apps/api/core';
import { Window } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { Menu, MenuItem } from '@tauri-apps/api/menu';
import DisplayPicker from './DisplayPicker';
import CaptureSettings, { CaptureConfig } from './CaptureSettings';
import CaptureShortcutOverlays from './CaptureShortcutOverlays';
import PermissionStatus from './PermissionStatus';
import { X, Monitor, Square, Camera, Mic, Volume2, Settings, ChevronDown } from 'lucide-react';
import { useRecordingStore } from '../stores/recording';
import { cn } from '../utils/cn';
import { useCaptureShortcuts } from '../hooks/useCaptureShortcuts';
type CaptureMode = 'display' | 'window' | 'area' | 'device';
type WebcamShape = 'circle' | 'roundrect';
const CaptureBar: React.FC = () => {
  const [captureMode, setCaptureMode] = useState<CaptureMode>('display');
  const [cameraEnabled, setCameraEnabled] = useState(false);
  const [micEnabled, setMicEnabled] = useState(false);
  const [systemAudioEnabled, setSystemAudioEnabled] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showDisplayPicker, setShowDisplayPicker] = useState(false);
  const [displays, setDisplays] = useState<any[]>([]);
  const [windows, setWindows] = useState<any[]>([]);
  const [windowsLoading, setWindowsLoading] = useState(true);
  const [devices, setDevices] = useState<any[]>([]);
  const [audioDevices, setAudioDevices] = useState<any>({ microphones: [], system_sources: [] });
  const [selectedDisplay, setSelectedDisplay] = useState<any>(null);
  const [selectedWindow, setSelectedWindow] = useState<string | null>(null);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [selectedCameraDevice, setSelectedCameraDevice] = useState<string | null>(null);
  const [selectedMicDevice, setSelectedMicDevice] = useState<string | null>(null);
  const [selectedSystemAudioDevice, setSelectedSystemAudioDevice] = useState<string | null>(null);
  const [webcamShape, setWebcamShape] = useState<WebcamShape>('circle');
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [showSpeakerNotes, setShowSpeakerNotes] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);
  const [captureConfig, setCaptureConfig] = useState<CaptureConfig>({
    includeCursor: true,
    cursorSize: 'normal',
    highlightClicks: false,
    fps: 60,
    outputResolution: 'match',
    encoder: 'auto',
    container: 'mp4'
  });
  const windowRefreshRef = useRef<Promise<any[]> | null>(null);
  const startInFlightRef = useRef(false);
  const stopInFlightRef = useRef(false);
  const { state, setRecordingState, stopRecording } = useRecordingStore();
  const isRecording = state === 'recording', isStarting = state === 'prerecord';
  const appWindows = windows.filter((w: any) => {
    const appName = (w.app_name || '').toLowerCase();
    const title = (w.title || '').toLowerCase();
    return appName !== 'tarantino' && title !== 'tarantino' && !title.includes('web inspector');
  });
  const windowModeReady = !windowsLoading && appWindows.length > 0;
  const selectedTargetReady = captureMode === 'window'
    ? windowModeReady && selectedWindow !== null
    : captureMode === 'display'
      ? selectedDisplay !== null
      : selectedDevice !== null;
  const canRecord = !startInFlightRef.current && !stopInFlightRef.current && !isStarting && (isRecording || selectedTargetReady);
  const isRecordableWindow = (windowInfo: any) => {
    const appName = (windowInfo.app_name || '').toLowerCase();
    const title = (windowInfo.title || '').toLowerCase();
    return appName !== 'tarantino' && title !== 'tarantino' && !title.includes('web inspector');
  };
  useEffect(() => {
    loadDevices();
  }, []);

  useEffect(() => {
    const unlisten = listen<string>('recording-stopped', async (event) => {
      if (event.payload) {
        await stopRecording(event.payload);
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [stopRecording]);

  const handleDragStart = async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.tagName === 'BUTTON' || target.closest('button') || target.closest('.capture-bar__input')) {
      return;
    }
    try {
      await Window.getCurrent().startDragging();
    } catch (error) {
      console.error('Failed to start dragging:', error);
    }
  };

  const loadDevices = async () => {
    try {
      const [displayList, deviceList, audioList] = await Promise.all([
        invoke<any[]>('get_displays'),
        invoke<any[]>('get_devices'),
        invoke<any>('get_audio_devices')
      ]);
      setDisplays(displayList);
      setDevices(deviceList);
      setAudioDevices(audioList);
      refreshWindows().catch((e) => console.error('Failed to refresh windows:', e));

      try {
        const selected = await invoke<any | null>('get_selected_display');
        if (selected) {
          setSelectedDisplay(selected);
          await invoke('capture_set_mode', { mode: 'desktop' });
          await invoke('capture_select_display', { id: selected.id });
        } else if (displayList.length > 0) {
          const primary = displayList.find((d: any) => d.is_primary) || displayList[0];
          setSelectedDisplay(primary);
          await invoke('capture_set_mode', { mode: 'desktop' });
          await invoke('capture_select_display', { id: primary.id });
        }
      } catch {
        if (displayList.length > 0) {
          const primary = displayList.find((d: any) => d.is_primary) || displayList[0];
          setSelectedDisplay(primary);
          await invoke('capture_set_mode', { mode: 'desktop' });
          await invoke('capture_select_display', { id: primary.id });
        }
      }
    } catch (err) {
      console.error('Failed to load devices:', err);
    }
  };

  const refreshWindows = async () => {
    if (windowRefreshRef.current) return windowRefreshRef.current;
    setWindowsLoading(true);
    windowRefreshRef.current = invoke<any[]>('refresh_windows')
      .then((list) => {
        setWindows(list);
        if (selectedWindow && !list.some((w: any) => w.id === selectedWindow && isRecordableWindow(w))) {
          setSelectedWindow(null);
        }
        return list;
      })
      .finally(() => { setWindowsLoading(false); windowRefreshRef.current = null; });
    return windowRefreshRef.current;
  };

  const handleModeChange = async (mode: CaptureMode) => {
    if (isStarting || isRecording) return;
    if (mode === 'window' && !windowModeReady) return;
    setCaptureMode(mode);
    const backendMode = mode === 'display' ? 'desktop' : mode;
    await invoke('capture_set_mode', { mode: backendMode });

    if (mode === 'display' && displays.length > 0) {
      setSelectedWindow(null);
      await selectDisplay(displays[0].id);
    } else if (mode === 'window') {
      if (!selectedWindow) showWindowMenu();
    } else if (mode === 'device' && devices.length > 0) {
      await selectDevice(devices[0].id);
    }
  };

  const selectDisplay = async (id: string) => {
    const display = displays.find(d => d.id === id);
    setSelectedDisplay(display);
    await invoke('capture_select_display', { id });
  };

  const selectWindow = async (id: string) => { setSelectedWindow(id); await invoke('capture_select_window', { id }); };

  const selectDevice = async (id: string) => { setSelectedDevice(id); await invoke('capture_select_device', { id }); };

  const handleCameraToggle = async () => {
    const enabled = !cameraEnabled;
    const previousCameraEnabled = cameraEnabled;
    const previousMicEnabled = micEnabled;
    setCameraError(null);
    setCameraEnabled(enabled);

    try {
      await invoke('input_set_camera', {
        enabled,
        deviceId: selectedCameraDevice,
        shape: webcamShape
      });

      if (enabled && !micEnabled) {
        setMicEnabled(true);
        await invoke('input_set_mic', {
          enabled: true,
          deviceId: selectedMicDevice || audioDevices.microphones[0]?.id || null
        });
      }
    } catch (error) {
      console.error('Failed to toggle camera:', error);
      const message = error instanceof Error ? error.message : String(error);
      if (enabled && !previousCameraEnabled) {
        invoke('input_set_camera', {
          enabled: false,
          deviceId: selectedCameraDevice,
          shape: webcamShape
        }).catch((disableError) => {
          console.error('Failed to roll back camera after error:', disableError);
        });
      }
      setCameraEnabled(previousCameraEnabled);
      setMicEnabled(previousMicEnabled);
      setCameraError(message);
      if (message.toLowerCase().includes('permission denied')) {
        const shouldOpenSettings = window.confirm('Camera access is turned off for Tarantino. Open Camera settings now?');
        if (shouldOpenSettings) {
          invoke('open_camera_preferences').catch((settingsError) => {
            console.error('Failed to open camera settings:', settingsError);
          });
        }
      }
    }
  };

  const handleCameraDeviceSelect = async (deviceId: string) => {
    const previousDeviceId = selectedCameraDevice;
    setSelectedCameraDevice(deviceId);
    if (cameraEnabled) {
      try {
        setCameraError(null);
        await invoke('input_set_camera', {
          enabled: true,
          deviceId,
          shape: webcamShape
        });
      } catch (error) {
        console.error('Failed to switch camera:', error);
        setSelectedCameraDevice(previousDeviceId);
        setCameraError(error instanceof Error ? error.message : String(error));
      }
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

  const handleWebcamShapeSelect = async (shape: WebcamShape) => {
    setWebcamShape(shape);
    if (cameraEnabled) {
      try {
        setCameraError(null);
        await invoke('input_set_camera', {
          enabled: true,
          deviceId: selectedCameraDevice,
          shape,
        });
      } catch (error) {
        console.error('Failed to update webcam shape:', error);
        setCameraError(error instanceof Error ? error.message : String(error));
      }
    }
  };
  const showCameraMenu = async () => {
    const cameraItems = await Promise.all(
      devices.map((device: any) =>
        MenuItem.new({
          text: `${selectedCameraDevice === device.id ? '✓ ' : '   '}${device.name || device.id}`,
          action: () => handleCameraDeviceSelect(device.id),
        })
      )
    );
    const shapeItems = await Promise.all([
      MenuItem.new({ text: 'Webcam shape', enabled: false }),
      MenuItem.new({
        text: `${webcamShape === 'circle' ? '✓ ' : '   '}Circle`,
        action: () => handleWebcamShapeSelect('circle'),
      }),
      MenuItem.new({
        text: `${webcamShape === 'roundrect' ? '✓ ' : '   '}Rounded rectangle`,
        action: () => handleWebcamShapeSelect('roundrect'),
      }),
    ]);
    const items = cameraItems.length > 0
      ? cameraItems.concat(shapeItems)
      : shapeItems;
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
    if (isStarting || isRecording || windowsLoading) return;
    const freshWindows = windows;
    const appWindows = freshWindows.filter((w: any) => {
      return isRecordableWindow(w);
    });

    if (appWindows.length === 0) {
      const noItems = await MenuItem.new({ text: 'No windows available', enabled: false });
      await (await Menu.new({ items: [noItems] })).popup();
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

  const startRecordingNow = async () => {
    if (!canRecord || startInFlightRef.current || stopInFlightRef.current || isRecording) return;
    startInFlightRef.current = true;
    setRecordingState('prerecord');
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
    const path = `/tmp/${timestamp}.${captureConfig.container}`;
    const targetType = captureMode === 'display' ? 'desktop' : (captureMode === 'window' ? 'window' : 'device');
    const targetId = targetType === 'desktop' ? (selectedDisplay?.id ?? '0') : (targetType === 'window' ? (selectedWindow ?? '0') : (selectedDevice ?? '0'));
    try {
      await invoke('capture_set_mode', { mode: targetType });
      if (targetType === 'desktop') await invoke('capture_select_display', { id: targetId });
      else if (targetType === 'window') await invoke('capture_select_window', { id: targetId });
      else await invoke('capture_select_device', { id: targetId });
      await invoke('record_start_new', { targetType, targetId,
        quality: 'High',
        includeCursor: captureConfig.includeCursor,
        includeMicrophone: micEnabled,
        includeSystemAudio: systemAudioEnabled,
        webcamShape,
        outputPath: path });
      setRecordingState('recording');
      await Window.getCurrent().hide();
    } catch (error) {
      console.error('Failed to start recording:', error);
      setRecordingState('idle');
      await Window.getCurrent().show().catch(() => {});
      await Window.getCurrent().setFocus().catch(() => {});
    } finally {
      startInFlightRef.current = false;
    }
  };

  const finishRecordingNow = async () => {
    if (stopInFlightRef.current || startInFlightRef.current) return;
    stopInFlightRef.current = true;
    const captureWindow = Window.getCurrent();
    try {
      await captureWindow.show();
      await captureWindow.setFocus();
      await invoke<string>('record_stop_instant_new');
      setRecordingState('review');
    } catch (error) {
      console.error('Failed to stop recording:', error);
      setRecordingState('recording');
    } finally {
      stopInFlightRef.current = false;
    }
  };

  const restartRecordingNow = async () => {
    if (!isRecording) {
      await startRecordingNow();
      return;
    }
    await invoke<string>('record_stop');
    setRecordingState('idle');
    await startRecordingNow();
  };

  const handleRecord = async () => {
    if (isRecording) await finishRecordingNow();
    else await startRecordingNow();
  };

  const handleExit = async () => {
    if (isRecording) {
      await invoke('record_stop');
    }
    await invoke('exit');
  };

  useCaptureShortcuts({
    isRecording,
    showShortcuts,
    showSpeakerNotes,
    onToggleShortcuts: () => setShowShortcuts((open) => !open),
    onCloseShortcuts: () => setShowShortcuts(false),
    onToggleSpeakerNotes: () => setShowSpeakerNotes((open) => !open),
    onCloseSpeakerNotes: () => setShowSpeakerNotes(false),
    onFinishRecording: finishRecordingNow,
    onStartRecording: startRecordingNow,
    onRestartRecording: restartRecordingNow,
  });

  return (
    <div className="capture-bar-pill" onMouseDown={handleDragStart}>
      <div
        className={cn('capture-bar__record', { recording: isRecording || isStarting, disabled: !canRecord })}
        onClick={handleRecord}
      >
        <div className="record-dot" />
      </div>

      <div className="capture-bar__modes">
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'display' })}
          disabled={isStarting || isRecording}
          onClick={() => {
            if (isStarting || isRecording) return;
            handleModeChange('display');
            setShowDisplayPicker(!showDisplayPicker);
          }}
          onMouseEnter={async () => {
            if (isStarting || isRecording) return;
            if (selectedDisplay) {
              try {
                await invoke('show_display_preview', { displayId: selectedDisplay.id });
              } catch (error) {
                console.error('Failed to show display preview:', error);
              }
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
          disabled={!windowModeReady || isStarting || isRecording}
          onClick={() => {
            if (!windowModeReady || isStarting || isRecording) return;
            captureMode === 'window' ? showWindowMenu() : handleModeChange('window');
          }}
          title={windowsLoading ? 'Loading windows' : windowModeReady ? 'Capture Window' : 'No windows available'}
        >
          <Square size={16} />
          <span style={{ maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{captureMode === 'window' && selectedWindow ? (windows.find((w: any) => w.id === selectedWindow)?.title || 'Window') : windowsLoading ? 'Loading...' : 'Window'}</span>
          {captureMode === 'window' && <ChevronDown size={12} />}
        </button>
      </div>

      <div className="capture-bar__inputs">
        <div className="capture-bar__input">
          <button
            className={cn('capture-bar__input-toggle', { active: cameraEnabled })}
            onClick={handleCameraToggle}
            title={cameraError || (cameraEnabled ? 'Disable Camera' : 'Enable Camera')}
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

      {cameraError && <div className="capture-bar__camera-error" title={cameraError}>Camera unavailable</div>}

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
            <PermissionStatus className="mb-4" />
            
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

      <CaptureShortcutOverlays
        showShortcuts={showShortcuts}
        showSpeakerNotes={showSpeakerNotes}
        onCloseShortcuts={() => setShowShortcuts(false)}
        onCloseSpeakerNotes={() => setShowSpeakerNotes(false)}
      />
    </div>
  );
};

export default CaptureBar;
