import React, { useState, useEffect, useRef } from 'react'; import { invoke } from '@tauri-apps/api/core';
import { Window } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { Menu, MenuItem } from '@tauri-apps/api/menu';
import DisplayPicker from './DisplayPicker';
import CaptureSettings, { CaptureConfig } from './CaptureSettings';
import VideoOutputLocation from './VideoOutputLocation';
import CaptureShortcutOverlays from './CaptureShortcutOverlays';
import PointerCaptureConsent from './PointerCaptureConsent';
import PermissionStatus from './PermissionStatus';
import { X, Monitor, Square, Camera, Mic, Volume2, Settings, ChevronDown, RotateCcw, AlertTriangle } from 'lucide-react';
import { useRecordingStore } from '../stores/recording';
import { cn } from '../utils/cn';
import { useCaptureShortcuts } from '../hooks/useCaptureShortcuts';
import { isLinuxRuntime } from '../utils/platform';
type CaptureMode = 'display' | 'window' | 'area' | 'device';
type WebcamShape = 'circle' | 'roundrect';
interface NativeMenuChoice {
  id: string;
  name?: string;
}
interface CameraDevice extends NativeMenuChoice {
  name: string;
  is_default?: boolean;
}
interface CaptureWindowInfo {
  id: string;
  title?: string;
  app_name?: string;
}
interface RecordingCapabilities {
  recordingAvailable: boolean;
  unavailableReason: string | null;
  usesSystemSourcePicker: boolean;
  displayPreviewAvailable: boolean;
  automaticZoomAvailable: boolean;
}
const optimisticNativeCapabilities: RecordingCapabilities = {
  recordingAvailable: true,
  unavailableReason: null,
  usesSystemSourcePicker: false,
  displayPreviewAvailable: true,
  automaticZoomAvailable: true,
};
const isRecordableWindow = (windowInfo: CaptureWindowInfo) => {
  const appName = (windowInfo.app_name || '').toLowerCase();
  const title = (windowInfo.title || '').toLowerCase();
  return appName !== 'tarantino' && title !== 'tarantino' && !title.includes('web inspector');
};
const createSelectionItems = <T extends NativeMenuChoice>(
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
const popupNativeMenu = async (items: Awaited<ReturnType<typeof MenuItem.new>>[]) => {
  const menu = await Menu.new({ items });
  await menu.popup();
};
const handleCaptureBarDrag = async (event: React.MouseEvent) => {
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
  const [windowPreparing, setWindowPreparing] = useState(false);
  const [devices, setDevices] = useState<CameraDevice[]>([]);
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
  const [recordingCapabilities, setRecordingCapabilities] = useState<RecordingCapabilities | null>(
    isLinuxRuntime ? null : optimisticNativeCapabilities,
  );
  const [showPointerConsent, setShowPointerConsent] = useState(false);
  const [checkingRecordingSupport, setCheckingRecordingSupport] = useState(false);
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
  const appWindows = windows.filter(isRecordableWindow);
  const windowModeReady = !windowsLoading && appWindows.length > 0;
  const portalWindow = recordingCapabilities?.usesSystemSourcePicker === true && appWindows.length === 1;
  const selectedTargetReady = captureMode === 'window'
    ? windowModeReady && selectedWindow !== null
    : captureMode === 'display'
      ? selectedDisplay !== null
      : selectedDevice !== null;
  const recordingAvailable = recordingCapabilities?.recordingAvailable === true;
  const canRecord = recordingAvailable && !startInFlightRef.current && !stopInFlightRef.current && !isStarting && !windowPreparing && (isRecording || selectedTargetReady);
  const checkRecordingSupport = async () => {
    setCheckingRecordingSupport(true);
    try {
      setRecordingCapabilities(await invoke<RecordingCapabilities>('get_recording_capabilities'));
    } catch (error) {
      setRecordingCapabilities({
        recordingAvailable: false,
        unavailableReason: `Could not verify recording support: ${String(error)}`,
        usesSystemSourcePicker: false,
        displayPreviewAvailable: false,
        automaticZoomAvailable: false,
      });
    } finally {
      setCheckingRecordingSupport(false);
    }
  };
  useEffect(() => {
    loadDevices();
    checkRecordingSupport();
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
  const loadDevices = async () => {
    try {
      const [displayList, deviceList, audioList] = await Promise.all([
        invoke<any[]>('get_displays'),
        invoke<any[]>('get_devices'),
        invoke<any>('get_audio_devices')
      ]);
      setDisplays(displayList);
      setDevices(deviceList);
      setSelectedCameraDevice((current) => {
        if (current && deviceList.some((device: CameraDevice) => device.id === current)) {
          return current;
        }
        return deviceList.find((device: CameraDevice) => device.is_default)?.id
          ?? deviceList[0]?.id
          ?? null;
      });
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

  const preparePortalWindow = async () => {
    if (!portalWindow || windowPreparing || isStarting || isRecording) return;
    const pickerWindow = appWindows[0];
    setWindowPreparing(true);
    try {
      await invoke('capture_prepare_window');
      setCaptureMode('window');
      await invoke('capture_set_mode', { mode: 'window' });
      await selectWindow(pickerWindow.id);
    } catch (error) {
      console.error('Failed to choose a window:', error);
      setSelectedWindow(null);
    } finally {
      setWindowPreparing(false);
    }
  };

  const handleModeChange = async (mode: CaptureMode) => {
    if (isStarting || isRecording) return;
    if (mode === 'window' && !windowModeReady) return;
    if (mode === 'window' && portalWindow) {
      await preparePortalWindow();
      return;
    }
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
    setCameraError(null);
    setCameraEnabled(enabled);

    try {
      await invoke('input_set_camera', {
        enabled,
        deviceId: selectedCameraDevice,
        deviceName: devices.find((device) => device.id === selectedCameraDevice)?.name ?? null,
        shape: webcamShape
      });

    } catch (error) {
      console.error('Failed to toggle camera:', error);
      const message = error instanceof Error ? error.message : String(error);
      if (enabled && !previousCameraEnabled) {
        invoke('input_set_camera', {
          enabled: false,
          deviceId: selectedCameraDevice,
          deviceName: devices.find((device) => device.id === selectedCameraDevice)?.name ?? null,
          shape: webcamShape
        }).catch((disableError) => {
          console.error('Failed to roll back camera after error:', disableError);
        });
      }
      setCameraEnabled(previousCameraEnabled);
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

  const handleCameraDeviceSelect = async (deviceId: string, deviceName?: string) => {
    const previousDeviceId = selectedCameraDevice;
    setSelectedCameraDevice(deviceId);
    if (cameraEnabled) {
      try {
        setCameraError(null);
        await invoke('input_set_camera', {
          enabled: true,
          deviceId,
          deviceName: deviceName ?? devices.find((device) => device.id === deviceId)?.name ?? null,
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
          deviceName: devices.find((device) => device.id === selectedCameraDevice)?.name ?? null,
          shape,
        });
      } catch (error) {
        console.error('Failed to update webcam shape:', error);
        setCameraError(error instanceof Error ? error.message : String(error));
      }
    }
  };
  const showCameraMenu = async () => {
    const cameraList = await invoke<CameraDevice[]>('get_devices');
    setDevices(cameraList);
    const currentSelection = selectedCameraDevice && cameraList.some((device) => device.id === selectedCameraDevice)
      ? selectedCameraDevice
      : cameraList.find((device) => device.is_default)?.id ?? cameraList[0]?.id ?? null;
    if (currentSelection !== selectedCameraDevice) {
      setSelectedCameraDevice(currentSelection);
    }
    const cameraItems = await createSelectionItems(
      cameraList,
      currentSelection,
      (device) => handleCameraDeviceSelect(device.id, device.name),
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
    await popupNativeMenu(cameraItems.length > 0 ? cameraItems.concat(shapeItems) : shapeItems);
  };

  const showMicMenu = async () => {
    if (audioDevices.microphones.length === 0) return;
    const items = await createSelectionItems(
      audioDevices.microphones,
      selectedMicDevice,
      (microphone) => handleMicDeviceSelect(microphone.id),
    );
    await popupNativeMenu(items);
  };

  const showSystemAudioMenu = async () => {
    const sources = audioDevices.system_sources || [];
    const choices = sources.length > 0 ? sources : [{ id: 'default', name: 'System Default' }];
    const items = await createSelectionItems(
      choices,
      selectedSystemAudioDevice,
      (source) => handleSystemAudioDeviceSelect(source.id),
    );
    await popupNativeMenu(items);
  };

  const showWindowMenu = async () => {
    if (isStarting || isRecording || windowsLoading) return;
    if (appWindows.length === 0) {
      const noItems = await MenuItem.new({ text: 'No windows available', enabled: false });
      await popupNativeMenu([noItems]);
      return;
    }

    const items = await createSelectionItems(
      appWindows,
      selectedWindow,
      (windowInfo) => {
        setSelectedWindow(windowInfo.id);
        selectWindow(windowInfo.id);
      },
      (windowInfo) => `${windowInfo.app_name ? `${windowInfo.app_name} — ` : ''}${windowInfo.title}`,
    );
    await popupNativeMenu(items);
  };

  const startRecordingNow = async (capturePointerEvents?: boolean) => {
    if (!canRecord || startInFlightRef.current || stopInFlightRef.current || isRecording) return;
    if (
      capturePointerEvents === undefined
      && recordingCapabilities?.usesSystemSourcePicker
      && recordingCapabilities.automaticZoomAvailable
    ) {
      setShowPointerConsent(true);
      return;
    }
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
        capturePointerEvents: capturePointerEvents ?? true,
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
    if (stopInFlightRef.current || startInFlightRef.current) return;
    stopInFlightRef.current = true;
    try {
      await invoke('record_restart_new');
      setRecordingState('recording');
      await Window.getCurrent().hide().catch(() => {});
    } catch (error) {
      console.error('Failed to restart recording:', error);
      setRecordingState('recording');
    } finally {
      stopInFlightRef.current = false;
    }
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
    <div className="capture-bar-pill" onMouseDown={handleCaptureBarDrag}>
      <div
        className={cn('capture-bar__record', { recording: isRecording || isStarting, disabled: !canRecord })}
        onClick={handleRecord}
        title={recordingCapabilities?.unavailableReason ?? (recordingCapabilities ? 'Start recording' : 'Checking recording support')}
      >
        <div className="record-dot" />
      </div>

      {isRecording && (
        <button
          className="capture-bar__restart"
          disabled={stopInFlightRef.current || startInFlightRef.current}
          onClick={restartRecordingNow}
          title="Restart Recording"
        >
          <RotateCcw size={16} />
        </button>
      )}

      {!recordingAvailable && recordingCapabilities ? (
        <button
          className="capture-bar__runtime-warning"
          onClick={() => setShowSettings(true)}
          title={recordingCapabilities.unavailableReason ?? 'Recording is unavailable'}
        >
          <AlertTriangle size={16} />
          <span>Recording unavailable</span>
        </button>
      ) : <div className="capture-bar__modes">
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'display' })}
          disabled={isStarting || isRecording}
          onClick={() => {
            if (isStarting || isRecording) return;
            handleModeChange('display');
            if (!recordingCapabilities?.usesSystemSourcePicker) {
              setShowDisplayPicker(!showDisplayPicker);
            }
          }}
          onMouseEnter={async () => {
            if (isStarting || isRecording) return;
            if (selectedDisplay && recordingCapabilities?.displayPreviewAvailable) {
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
          title={recordingCapabilities?.usesSystemSourcePicker ? 'Capture the current display; your desktop confirms screen access when recording starts' : 'Capture Display'}
        >
          <Monitor size={16} />
          <span>{selectedDisplay ? selectedDisplay.name : 'Display'}</span>
        </button>
        <button
          className={cn('capture-bar__mode', { active: captureMode === 'window' })}
          disabled={!windowModeReady || windowPreparing || isStarting || isRecording}
          onClick={() => {
            if (!windowModeReady || windowPreparing || isStarting || isRecording) return;
            if (portalWindow) {
              preparePortalWindow();
            } else {
              captureMode === 'window' ? showWindowMenu() : handleModeChange('window');
            }
          }}
          title={windowPreparing
            ? 'Choosing window'
            : windowsLoading
            ? 'Loading windows'
            : windowModeReady
              ? recordingCapabilities?.usesSystemSourcePicker
                ? 'Choose Window, then press Record to open the secure system window picker'
                : 'Capture Window'
              : 'No windows available'}
        >
          <Square size={16} />
          <span style={{ maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{windowPreparing ? 'Choosing...' : captureMode === 'window' && selectedWindow ? (portalWindow ? 'Selected window' : (windows.find((w: any) => w.id === selectedWindow)?.title || 'Window')) : windowsLoading ? 'Loading...' : 'Window'}</span>
          {captureMode === 'window' && <ChevronDown size={12} />}
        </button>
      </div>}

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
            <VideoOutputLocation />
            {recordingCapabilities?.unavailableReason && (
              <div className="capture-bar__runtime-detail" role="alert">
                <AlertTriangle size={16} />
                <div>
                  <strong>Recording unavailable</strong>
                  <span>{recordingCapabilities.unavailableReason}</span>
                  <button
                    className="capture-bar__runtime-retry"
                    disabled={checkingRecordingSupport}
                    onClick={checkRecordingSupport}
                  >
                    {checkingRecordingSupport ? 'Checking…' : 'Check again'}
                  </button>
                </div>
              </div>
            )}

            {recordingCapabilities?.usesSystemSourcePicker ? (
              <div className="capture-bar__portal-notice">
                Display capture defaults to the display containing Tarantino. For Window capture,
                choose the window option first; Record then opens your desktop's secure window picker.
                {!recordingCapabilities.automaticZoomAvailable && (
                  <span className="capture-bar__portal-limit">
                    Automatic click zoom needs permission to read a compatible mouse device. Recording
                    and manual zoom editing remain available.
                  </span>
                )}
              </div>
            ) : <PermissionStatus className="mb-4" />}
            
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
      {showPointerConsent && (
        <PointerCaptureConsent
          onEnable={() => {
            setShowPointerConsent(false);
            void startRecordingNow(true);
          }}
          onSkip={() => {
            setShowPointerConsent(false);
            void startRecordingNow(false);
          }}
        />
      )}
    </div>
  );
};

export default CaptureBar;
