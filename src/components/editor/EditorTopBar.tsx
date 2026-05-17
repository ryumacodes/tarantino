import React, { useState, useRef, useEffect } from 'react';
import {
  Settings,
  Download,
  ChevronDown,
  Wand2,
  Square,
  Sparkles,
  Zap,
  Pencil,
  Check
} from 'lucide-react';
import '../../styles/dracula-theme.css';
import '../../styles/editor-top-bar.css';

interface EditorTopBarProps {
  projectName: string;
  onProjectNameChange: (name: string) => void;
  onExport: () => void;
  onClose: () => void;
  isExporting?: boolean;
  exportProgress?: number;
}

const EditorTopBar: React.FC<EditorTopBarProps> = ({
  projectName,
  onProjectNameChange,
  onExport,
  onClose,
  isExporting = false,
  exportProgress = 0
}) => {
  const [presetsOpen, setPresetsOpen] = useState(false);
  const [selectedPreset, setSelectedPreset] = useState('Auto');
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [editedTitle, setEditedTitle] = useState(projectName);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isEditingTitle && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isEditingTitle]);

  useEffect(() => {
    setEditedTitle(projectName);
  }, [projectName]);

  const handleStartEdit = () => {
    if (isExporting) return;
    setEditedTitle(projectName);
    setIsEditingTitle(true);
  };

  const handleSaveTitle = () => {
    const trimmedTitle = editedTitle.trim();
    if (trimmedTitle && trimmedTitle !== projectName) {
      onProjectNameChange(trimmedTitle);
    } else {
      setEditedTitle(projectName);
    }
    setIsEditingTitle(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSaveTitle();
    } else if (e.key === 'Escape') {
      setEditedTitle(projectName);
      setIsEditingTitle(false);
    }
  };

  const presets = [
    {
      id: 'auto',
      name: 'Auto',
      icon: Wand2,
      description: 'Smart zooms + polish',
      features: ['Auto-zoom', 'Cursor smooth', 'Background']
    },
    {
      id: 'minimal',
      name: 'Minimal',
      icon: Square,
      description: 'Clean recording only',
      features: ['No effects', 'Raw quality']
    },
    {
      id: 'enhanced',
      name: 'Enhanced',
      icon: Sparkles,
      description: 'Professional polish',
      features: ['Smart zoom', 'Motion blur', 'Background', 'Audio enhance']
    },
    {
      id: 'presentation',
      name: 'Presentation',
      icon: Zap,
      description: 'Perfect for demos',
      features: ['Auto-zoom', 'Click highlights', 'Smooth cursor', 'Wallpaper']
    }
  ];

  const currentPreset = presets.find(p => p.id === selectedPreset.toLowerCase()) || presets[0];

  return (
    <div className="editor-top-bar" data-tauri-drag-region>
      {/* Left — space for native traffic lights */}
      <div className="top-bar-left" data-tauri-drag-region>
        <div className="traffic-light-spacer" />
      </div>

      {/* Center — Project title */}
      <div className="top-bar-center" data-tauri-drag-region>
        <div className="project-info">
          {isEditingTitle ? (
            <div className="project-name-edit">
              <input
                ref={inputRef}
                type="text"
                value={editedTitle}
                onChange={(e) => setEditedTitle(e.target.value)}
                onBlur={handleSaveTitle}
                onKeyDown={handleKeyDown}
                className="project-name-input"
                disabled={isExporting}
              />
              <button
                className="edit-confirm-btn"
                onClick={handleSaveTitle}
                title="Save"
                disabled={isExporting}
              >
                <Check size={14} />
              </button>
            </div>
          ) : (
            <div className="project-name-display" data-tauri-drag-region>
              <h1 className="project-name" data-tauri-drag-region>{projectName}</h1>
              <button
                className="edit-title-btn"
                onClick={handleStartEdit}
                title="Edit title"
                disabled={isExporting}
              >
                <Pencil size={12} />
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Right — Preset + Settings + Export */}
      <div className="top-bar-right">
        <div className="presets-container">
          <button
            className={`presets-button ${presetsOpen ? 'active' : ''}`}
            onClick={() => !isExporting && setPresetsOpen(!presetsOpen)}
            disabled={isExporting}
          >
            <currentPreset.icon size={14} />
            <span>{currentPreset.name}</span>
            <ChevronDown size={12} className={`chevron ${presetsOpen ? 'rotated' : ''}`} />
          </button>

          {presetsOpen && (
            <>
              <div className="presets-backdrop" onClick={() => setPresetsOpen(false)} />
              <div className="presets-dropdown">
                <div className="presets-header">
                  <h3>Export Presets</h3>
                  <p>Choose how to process your recording</p>
                </div>
                <div className="presets-list">
                  {presets.map((preset) => {
                    const Icon = preset.icon;
                    const isSelected = preset.id === selectedPreset.toLowerCase();
                    return (
                      <button
                        key={preset.id}
                        className={`preset-item ${isSelected ? 'selected' : ''}`}
                        onClick={() => {
                          setSelectedPreset(preset.name);
                          setPresetsOpen(false);
                        }}
                        disabled={isExporting}
                      >
                        <div className="preset-icon">
                          <Icon size={18} />
                        </div>
                        <div className="preset-content">
                          <div className="preset-name">{preset.name}</div>
                          <div className="preset-description">{preset.description}</div>
                          <div className="preset-features">
                            {preset.features.map((feature, index) => (
                              <span key={index} className="feature-tag">{feature}</span>
                            ))}
                          </div>
                        </div>
                        {isSelected && (
                          <div className="preset-selected-indicator">
                            <div className="selected-dot" />
                          </div>
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            </>
          )}
        </div>

        <button className="editor-btn editor-btn--ghost editor-btn--icon" title="Settings">
          <Settings size={16} />
        </button>

        <button
          className={`export-button ${isExporting ? 'exporting' : ''}`}
          onClick={onExport}
          disabled={isExporting}
        >
          {isExporting ? (
            <>
              <div className="export-progress">
                <div
                  className="export-progress-fill"
                  style={{ width: `${exportProgress}%` }}
                />
              </div>
              <span>Exporting... {Math.round(exportProgress)}%</span>
            </>
          ) : (
            <>
              <Download size={14} />
              <span>Export</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
};

export default EditorTopBar;
