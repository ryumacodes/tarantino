import React, { useState, useEffect } from 'react';
import {
  Volume2,
  Download,
  MousePointer,
  Sparkles,
  Monitor,
  Target,
  Film
} from 'lucide-react';
import CursorSettingsPanel from './CursorSettingsPanel';
import { ZoomTab, ClipsTab, BackgroundTab, MotionTab, AudioTab, ExportTab, type TabType } from './properties';
import '../../styles/dracula-theme.css';
import '../../styles/properties-panel.css';

interface PropertiesPanelProps {
  onShowMouseOverlay?: (show: boolean) => void;
  showMouseOverlay?: boolean;
  isExporting?: boolean;
}

const PropertiesPanel: React.FC<PropertiesPanelProps> = ({
  onShowMouseOverlay,
  showMouseOverlay = true,
  isExporting = false
}) => {
  const [activeTab, setActiveTab] = useState<TabType>('zoom');

  // Keyboard shortcuts for panel navigation
  useEffect(() => {
    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey) {
        const keyMap: Record<string, TabType> = {
          '1': 'zoom',
          '2': 'clips',
          '3': 'background',
          '4': 'cursor',
          '5': 'motion',
          '6': 'audio',
          '7': 'export'
        };

        if (keyMap[e.key]) {
          e.preventDefault();
          setActiveTab(keyMap[e.key]);
        }
      }
    };

    document.addEventListener('keydown', handleKeyPress);
    return () => document.removeEventListener('keydown', handleKeyPress);
  }, []);

  const tabs = [
    { id: 'zoom', name: 'Zoom', icon: Target, description: 'Click-based zoom effects' },
    { id: 'clips', name: 'Clips', icon: Film, description: 'Clip speed & timing' },
    { id: 'background', name: 'Background', icon: Monitor, description: 'Wallpapers & gradients' },
    { id: 'cursor', name: 'Cursor', icon: MousePointer, description: 'Cursor enhancement' },
    { id: 'motion', name: 'Motion', icon: Sparkles, description: 'Animation & easing' },
    { id: 'audio', name: 'Audio', icon: Volume2, description: 'Audio processing' },
    { id: 'export', name: 'Export', icon: Download, description: 'Export settings' }
  ];

  const renderCursorTab = () => (
    <CursorSettingsPanel
      showMouseOverlay={showMouseOverlay}
      onShowMouseOverlay={onShowMouseOverlay}
      isExporting={isExporting}
    />
  );

  const renderTabContent = () => {
    switch (activeTab) {
      case 'zoom':
        return <ZoomTab isExporting={isExporting} />;
      case 'clips':
        return <ClipsTab isExporting={isExporting} />;
      case 'background':
        return <BackgroundTab isExporting={isExporting} />;
      case 'cursor':
        return renderCursorTab();
      case 'motion':
        return <MotionTab isExporting={isExporting} />;
      case 'audio':
        return <AudioTab isExporting={isExporting} />;
      case 'export':
        return <ExportTab isExporting={isExporting} />;
      default:
        return null;
    }
  };

  return (
    <div className="properties-panel">
      {/* Main Content Area */}
      <div className="panel-main">
        <div className="panel-header">
          <h2 className="panel-title">
            {tabs.find(tab => tab.id === activeTab)?.name || 'Settings'}
          </h2>
          <p className="panel-description">
            {tabs.find(tab => tab.id === activeTab)?.description || ''}
          </p>
        </div>

        <div className="panel-content editor-scrollbar">
          {renderTabContent()}
        </div>
      </div>

      {/* Icon Navigation on Right */}
      <div className="panel-sidebar">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          return (
            <button
              key={tab.id}
              className={`sidebar-tab ${activeTab === tab.id ? 'active' : ''}`}
              onClick={() => setActiveTab(tab.id as TabType)}
              title={`${tab.name} - ${tab.description}`}
            >
              <Icon size={18} />
            </button>
          );
        })}
      </div>
    </div>
  );
};

export default PropertiesPanel;
