import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

const time = document.getElementById('time')!;
const message = document.getElementById('message')!;
const stop = document.getElementById('stop') as HTMLButtonElement;
const restart = document.getElementById('restart') as HTMLButtonElement;
let busy = false;
let disposed = false;
let poll: ReturnType<typeof setTimeout> | undefined;

async function refresh() {
  if (disposed) return;
  try {
    const status = await invoke<{ is_recording: boolean; tray_state: { recording_duration: string | null } }>('get_recording_status');
    if (disposed) return;
    time.textContent = status.tray_state.recording_duration ?? '00:00:00';
  } catch (error) {
    if (!busy) { message.textContent = `Status unavailable: ${String(error)}`; message.className = 'error'; }
  } finally {
    if (!disposed) poll = setTimeout(refresh, 1000);
  }
}
async function perform(command: 'hud_stop' | 'record_restart_new') {
  if (busy) return;
  busy = true;
  stop.disabled = restart.disabled = true;
  message.className = '';
  message.textContent = command === 'hud_stop' ? 'Stopping…' : 'Restarting…';
  try {
    await invoke(command);
    message.textContent = command === 'hud_stop' ? 'Finishing recording…' : 'Recording';
  } catch (error) {
    message.textContent = `Could not ${command === 'hud_stop' ? 'stop' : 'restart'}: ${String(error)}`;
    message.className = 'error';
  } finally {
    busy = false;
    stop.disabled = restart.disabled = false;
  }
}
stop.addEventListener('click', () => void perform('hud_stop'));
restart.addEventListener('click', () => void perform('record_restart_new'));
time.addEventListener('pointerdown', event => {
  if (event.button === 0) void getCurrentWindow().startDragging();
});
window.addEventListener('pagehide', () => { disposed = true; clearTimeout(poll); });
void refresh();
