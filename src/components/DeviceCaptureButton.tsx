import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { MenuItem } from '@tauri-apps/api/menu';
import { Smartphone } from 'lucide-react';
import { createSelectionItems, popupNativeMenu } from './captureBarHelpers';

interface Device { id: string; name: string }
export default function DeviceCaptureButton({ active, disabled, onSelect, onError }: {
  active: boolean; disabled: boolean;
  onSelect: (id: string) => Promise<void>; onError: (message: string) => void;
}) {
  const [selected, setSelected] = useState<Device | null>(null);
  const [loading, setLoading] = useState(false);
  const showDevices = async () => {
    if (disabled || loading) return;
    setLoading(true);
    try {
      const devices = await invoke<Device[]>('get_screen_devices');
      const items = await createSelectionItems(devices, selected?.id ?? null, device => {
        onSelect(device.id).then(() => setSelected(device)).catch(error => onError(String(error)));
      });
      if (!items.length) items.push(await MenuItem.new({
        text: 'Connect an iPhone or iPad by USB, unlock it and tap Trust. Click Device to refresh.', enabled: false,
      }));
      await popupNativeMenu(items);
    } catch (error) { onError(`Could not list devices: ${String(error)}`); }
    finally { setLoading(false); }
  };
  return <button className={`capture-bar__mode${active ? ' active' : ''}`}
    disabled={disabled || loading} onClick={showDevices} title="Record an iPhone or iPad screen over USB">
    <Smartphone size={16} /><span>{loading ? 'Searching…' : active && selected ? selected.name : 'Device'}</span>
  </button>;
}
