import React, { useState, useEffect, useRef } from 'react'; import { invoke } from '@tauri-apps/api/core';
import { Window } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { MenuItem } from '@tauri-apps/api/menu';
import DisplayPicker, { type Display } from './DisplayPicker';
import DeviceCaptureButton from './DeviceCaptureButton';
import type { CaptureConfig } from './CaptureSettings';
import CaptureSettingsPanel from './CaptureSettingsPanel';
import { RecordingCountdown } from './RecordingCountdown';
import { useRecordingCountdown } from '../hooks/useRecordingCountdown';
import CaptureShortcutOverlays from './CaptureShortcutOverlays';
import PointerCaptureConsent from './PointerCaptureConsent';
import { X, Monitor, Square, Camera, Mic, Volume2, Settings, ChevronDown, RotateCcw, AlertTriangle } from 'lucide-react';
import { useRecordingStore } from '../stores/recording';
import { cn } from '../utils/cn';
import { useCaptureShortcuts } from '../hooks/useCaptureShortcuts';
import { isLinuxRuntime } from '../utils/platform';
import { optimisticNativeCapabilities, type RecordingCapabilities } from '../utils/recordingCapabilities';
import { type CaptureMode, type WebcamShape, type CameraDevice, type CaptureWindowInfo, type AudioDevices, isRecordableWindow, createSelectionItems, popupNativeMenu, handleCaptureBarDrag, exitCaptureBar, initialCaptureConfig } from './captureBarHelpers';
const CaptureBar: React.FC = () => {
  const countdown = useRecordingCountdown();
  const [captureMode, setCaptureMode] = useState<CaptureMode>('display');
  const [cameraEnabled, setCameraEnabled] = useState(false);
  const [micEnabled, setMicEnabled] = useState(false);
  const [systemAudioEnabled, setSystemAudioEnabled] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showDisplayPicker, setShowDisplayPicker] = useState(false);
  const [displays, setDisplays] = useState<Display[]>([]);
  const [windows, setWindows] = useState<CaptureWindowInfo[]>([]);
  const [windowsLoading, setWindowsLoading] = useState(true);
  const [windowPreparing, setWindowPreparing] = useState(false);
  const [devices, setDevices] = useState<CameraDevice[]>([]);
  const [audioDevices, setAudioDevices] = useState<AudioDevices>({ microphones: [], system_sources: [] });
  const [selectedDisplay, setSelectedDisplay] = useState<Display | null>(null);
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
  const [captureConfig, setCaptureConfig] = useState<CaptureConfig>(initialCaptureConfig);
  const windowRefreshRef = useRef<Promise<CaptureWindowInfo[]> | null>(null);
  const startInFlightRef = useRef(false);
  const stopInFlightRef = useRef(false);
  const { state, setRecordingState, stopRecording, recordingError, failRecording, confirmRecordingStarted } = useRecordingStore();
  const isRecording = state === 'recording', isStarting = state === 'prerecord';
  const appWindows = windows.filter(isRecordableWindow);
  const windowModeReady = !windowsLoading && appWindows.length > 0;
  const portalWindow = recordingCapabilities?.usesSystemSourcePicker === true && appWindows.length === 1;
  const selectedTargetReady = captureMode === 'window'
    ? windowModeReady && selectedWindow !== null
    : captureMode === 'display'
      ? displays.some(display => display.id === selectedDisplay?.id)
      : selectedDevice !== null;
  const systemAudioAvailable = captureMode !== 'device' && (recordingCapabilities?.systemAudioAvailable ?? !isLinuxRuntime);
  const systemAudioUnavailableReason = captureMode === 'device' ? "USB device audio is not available yet." : "System audio requires the GStreamer PulseAudio and WAV plugins.";
  const recordingAvailable = recordingCapabilities?.recordingAvailable === true;
  const canRecord = recordingAvailable && !startInFlightRef.current && !stopInFlightRef.current && !isStarting && !windowPreparing && (isRecording || selectedTargetReady);
  const checkRecordingSupport = async () => {
    setCheckingRecordingSupport(true);
    try {
      const capabilities = await invoke<RecordingCapabilities>('get_recording_capabilities');
      setRecordingCapabilities(capabilities);
      if (capabilities.recordingAvailable) await loadDevices();
    } catch (error) {
      setRecordingCapabilities({
        recordingAvailable: false,
        unavailableReason: `Could not verify recording support: ${String(error)}`,
        usesSystemSourcePicker: false,
        displayPreviewAvailable: false,
        automaticZoomAvailable: false,
        systemAudioAvailable: !isLinuxRuntime,
      });
    } finally {
      setCheckingRecordingSupport(false);
    }
  };
  useEffect(() => {
    checkRecordingSupport();
  }, []);
  useEffect(() => {
    const unlisten = listen<string>('recording-stopped', async (event) => {
      if (event.payload) {
        await stopRecording(event.payload);
      }
    });
    const unlistenFailure = listen<string>('recording:failed', (event) => failRecording(event.payload));
    return () => {
      unlisten.then(fn => fn());
      unlistenFailure.then(fn => fn());
    };
  }, [stopRecording, failRecording]);
  const loadDevices = async () => {
    try {
      const [displayList, deviceList, audioList] = await Promise.all([
        invoke<Display[]>('get_displays'),
        invoke<CameraDevice[]>('get_devices'),
        invoke<AudioDevices>('get_audio_devices')
      ]);
      setDisplays(displayList);
      setDevices(deviceList);
      setSelectedCameraDevice((current) => {
        if (current && deviceList.some((device) => device.id === current)) {
          return current;
        }
        return deviceList.find((device) => device.is_default)?.id
          ?? deviceList[0]?.id
          ?? null;
      });
      setAudioDevices(audioList);
      refreshWindows().catch((e) => console.error('Failed to refresh windows:', e));
      const selected = await invoke<Display | null>('get_selected_display').catch(() => null)
        ?? displayList.find(display => display.is_primary)
        ?? displayList[0];
      if (selected) {
        setSelectedDisplay(selected);
        await invoke('capture_set_mode', { mode: 'desktop' });
        await invoke('capture_select_display', { id: selected.id });
      }
    } catch (err) {
      console.error('Failed to load devices:', err);
    }
  };
  const refreshWindows = async () => {
    if (windowRefreshRef.current) return windowRefreshRef.current;
    setWindowsLoading(true);
    windowRefreshRef.current = invoke<CaptureWindowInfo[]>('refresh_windows')
      .then((list) => {
        setWindows(list);
        setSelectedWindow(current => current && list.some((w) => w.id === current && isRecordableWindow(w)) ? current : null);
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
    }
  };

  const selectDisplay = async (id: string) => {
    const display = displays.find(d => d.id === id);
    setSelectedDisplay(display ?? null);
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
    if (!systemAudioAvailable) return;
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
    if (!systemAudioAvailable) return;
    const sources = audioDevices.system_sources;
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

    const picker = document.getElementById('capture-window-select') as HTMLSelectElement | null;
    picker?.focus();
    try { picker?.showPicker(); } catch { /* Keyboard selection remains available. */ }
  };

  const startRecordingNow = async (capturePointerEvents?: boolean) => {
    if (!canRecord || startInFlightRef.current || stopInFlightRef.current || isRecording) return;
    if (
      capturePointerEvents === undefined
      && !isLinuxRuntime
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
      setShowSettings(false);
      if (!await countdown.start()) { setRecordingState('idle'); return; }
      await invoke('record_start_new', { targetType, targetId,
        quality: 'High', frameRate: captureConfig.fps,
        includeCursor: captureConfig.includeCursor,
        captureShortcuts: captureConfig.captureShortcuts === true,
        includeMicrophone: micEnabled,
        includeSystemAudio: systemAudioAvailable && systemAudioEnabled,
        capturePointerEvents: capturePointerEvents ?? true,
        webcamShape,
        outputPath: path });
      if (!confirmRecordingStarted()) return;
      if (!isLinuxRuntime) await Window.getCurrent().hide();
    } catch (error) {
      console.error('Failed to start recording:', error);
      failRecording(String(error));
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
      if (!useRecordingStore.getState().recordingError) setRecordingState('recording');
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
    setRecordingState('prerecord');
    try {
      await invoke('record_restart_new');
      if (!confirmRecordingStarted()) return;
      if (!isLinuxRuntime) await Window.getCurrent().hide().catch(() => {});
    } catch (error) {
      console.error('Failed to restart recording:', error);
      if (!useRecordingStore.getState().recordingError) setRecordingState('recording');
    } finally {
      stopInFlightRef.current = false;
    }
  };

  const handleRecord = async () => { if (isRecording) await finishRecordingNow(); else await startRecordingNow(); };

  useCaptureShortcuts({
    isRecording, isStarting, showShortcuts, showSpeakerNotes,
    onChooseDisplay: async () => { await handleModeChange('display'); setShowDisplayPicker(true); },
    onChooseWindow: async () => { if (captureMode === 'window') await showWindowMenu(); else await handleModeChange('window'); },
    onShowCaptureControls: () => { setShowSettings(false); setShowShortcuts(false); setShowSpeakerNotes(false); },
    onOpenSettings: () => setShowSettings(true), onOpenShortcuts: () => setShowShortcuts(true),
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
      <RecordingCountdown remaining={countdown.remaining} cancel={countdown.cancel} />
      {recordingError && <button className="capture-bar__runtime-warning" role="alert"
        title={recordingError} onClick={() => alert(recordingError)}>Recording failed — details</button>}
      <button type="button"
        className={cn('capture-bar__record', { recording: isRecording || isStarting, disabled: !canRecord })}
        disabled={!canRecord}
        aria-label={isRecording ? 'Stop recording' : 'Start recording'}
        onClick={handleRecord}
        title={recordingCapabilities?.unavailableReason ?? (recordingCapabilities ? 'Start recording' : 'Checking recording support')}
      >
        <div className="record-dot" />
      </button>

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
          disabled={displays.length === 0 || isStarting || isRecording}
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
        {portalWindow ? <button className={cn('capture-bar__mode', { active: captureMode === 'window' })}
          disabled={!windowModeReady || windowPreparing || isStarting || isRecording}
          onClick={preparePortalWindow}><Square size={16} />Choose Window</button> :
          <select id="capture-window-select" aria-label="Capture window"
            className={cn('capture-bar__mode', { active: captureMode === 'window' })}
            style={{ maxWidth: 185, appearance: 'auto' }}
            value={captureMode === 'window' ? selectedWindow ?? '' : ''}
            disabled={isStarting || isRecording}
            onFocus={() => { void refreshWindows().catch(error => console.error('Failed to refresh windows:', error)); }}
            onMouseDown={event => {
              event.stopPropagation();
              void refreshWindows().catch(error => console.error('Failed to refresh windows:', error));
            }}
            onChange={async event => {
              const id = event.target.value;
              if (!id) return;
              try {
                await invoke('capture_set_mode', { mode: 'window' });
                await selectWindow(id);
                setCaptureMode('window');
              } catch (error) { failRecording(`Could not select window: ${String(error)}`); }
            }}>
            <option value="" disabled>{windowsLoading ? 'Loading windows…' : 'Choose Window'}</option>
            {appWindows.map(windowInfo => <option key={windowInfo.id} value={windowInfo.id}>
              {windowInfo.app_name ? `${windowInfo.app_name} — ` : ''}{windowInfo.title}
            </option>)}
          </select>}
        {!isLinuxRuntime && <DeviceCaptureButton active={captureMode === 'device'} disabled={isStarting || isRecording}
          onError={failRecording} onSelect={async id => { await selectDevice(id); await handleModeChange('device'); }} />}
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
            disabled={!systemAudioAvailable}
            title={!systemAudioAvailable ? systemAudioUnavailableReason : systemAudioEnabled ? 'Disable System Audio' : 'Enable System Audio'}
          >
            <Volume2 size={16} />
          </button>
          <button
            className="capture-bar__input-select"
            onClick={showSystemAudioMenu}
            disabled={!systemAudioAvailable}
            title={systemAudioAvailable ? "Select System Audio" : systemAudioUnavailableReason}
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
        onClick={() => exitCaptureBar(isRecording)}
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
              setSelectedDisplay(display ?? null);
              selectDisplay(display.id);
              setShowDisplayPicker(false);
            }}
          />
        </div>
      )}
      
      {showSettings && (
        <CaptureSettingsPanel
          recordingCapabilities={recordingCapabilities}
          checkingRecordingSupport={checkingRecordingSupport}
          checkRecordingSupport={checkRecordingSupport}
          captureConfig={captureConfig}
          setCaptureConfig={setCaptureConfig}
          selectedDisplay={selectedDisplay}
        />
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
