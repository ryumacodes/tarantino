import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Monitor, Check, RefreshCw } from 'lucide-react';
import { cn } from '../utils/cn';

interface Display {
  id: string;
  name: string;
  width: number;
  height: number;
  scale_factor: number;
  refresh_rate: number;
  is_primary: boolean;
  thumbnail?: string; // base64 encoded thumbnail
}

interface DisplayPickerProps {
  onSelect: (display: Display) => void;
  selectedId?: string;
  compact?: boolean;
}

const DisplayPicker: React.FC<DisplayPickerProps> = ({ 
  onSelect, 
  selectedId,
  compact = false 
}) => {
  const [displays, setDisplays] = useState<Display[]>([]);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(false);

  const loadDisplays = async () => {
    setLoading(true);
    try {
      const displayList = await invoke<Display[]>('get_displays_with_thumbnails');
      setDisplays(displayList);
      
      // Auto-select primary if nothing selected
      if (!selectedId && displayList.length > 0) {
        const primary = displayList.find(d => d.is_primary) || displayList[0];
        onSelect(primary);
      }
    } catch (error) {
      console.error('Failed to load displays:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDisplays();
  }, []);

  const formatResolution = (display: Display) => {
    const physicalWidth = Math.round(display.width * display.scale_factor);
    const physicalHeight = Math.round(display.height * display.scale_factor);
    
    if (display.scale_factor !== 1) {
      return `${display.width}×${display.height} (${physicalWidth}×${physicalHeight})`;
    }
    return `${display.width}×${display.height}`;
  };

  if (compact) {
    const selected = displays.find(d => d.id === selectedId);
    
    return (
      <div className="display-picker-compact">
        <button 
          className="display-picker-compact__trigger"
          onClick={() => setExpanded(!expanded)}
        >
          <Monitor size={16} />
          <span>{selected?.name || 'Select Display'}</span>
          <span className="display-picker-compact__meta">
            {selected && formatResolution(selected)}
          </span>
        </button>
        
        {expanded && (
          <div className="display-picker-compact__dropdown">
            {displays.map(display => (
              <button
                key={display.id}
                className={cn('display-picker-compact__item', {
                  selected: display.id === selectedId
                })}
                onClick={() => {
                  onSelect(display);
                  setExpanded(false);
                }}
              >
                <div className="display-picker-compact__item-info">
                  <span className="display-picker-compact__item-name">
                    {display.name}
                    {display.is_primary && ' (Primary)'}
                  </span>
                  <span className="display-picker-compact__item-meta">
                    {formatResolution(display)} @ {display.refresh_rate}Hz
                  </span>
                </div>
                {display.id === selectedId && <Check size={14} />}
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="display-picker">
      <div className="display-picker__header">
        <h3>Select Display</h3>
        <button 
          className="display-picker__refresh"
          onClick={loadDisplays}
          disabled={loading}
        >
          <RefreshCw size={16} className={loading ? 'spinning' : ''} />
        </button>
      </div>
      
      <div className="display-picker__grid">
        {displays.map(display => (
          <button
            key={display.id}
            className={cn('display-picker__item', {
              selected: display.id === selectedId
            })}
            onClick={() => onSelect(display)}
          >
            <div className="display-picker__thumbnail">
              {display.thumbnail ? (
                <img 
                  src={`data:image/png;base64,${display.thumbnail}`} 
                  alt={display.name}
                />
              ) : (
                <div className="display-picker__thumbnail-placeholder">
                  <Monitor size={32} />
                </div>
              )}
              {display.id === selectedId && (
                <div className="display-picker__selected-badge">
                  <Check size={16} />
                </div>
              )}
            </div>
            
            <div className="display-picker__info">
              <div className="display-picker__name">
                {display.name}
                {display.is_primary && (
                  <span className="display-picker__primary">Primary</span>
                )}
              </div>
              <div className="display-picker__specs">
                {formatResolution(display)}
              </div>
              <div className="display-picker__specs">
                {display.refresh_rate}Hz
                {display.scale_factor !== 1 && ` • ${display.scale_factor}x scale`}
              </div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
};

export default DisplayPicker;