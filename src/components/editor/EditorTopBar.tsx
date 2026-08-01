import React, { useState, useRef, useEffect } from 'react';
import {
  Settings,
  Download,
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

  return (
    <div className="editor-top-bar" data-tauri-drag-region>
      <div className="top-bar-left" data-tauri-drag-region>
        <div className="traffic-light-spacer" />
      </div>

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

      <div className="top-bar-right">
        {/* TODO: Add functional export presets. */}
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
