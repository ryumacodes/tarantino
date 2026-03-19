// Properties Panel Types

export type TabType = 'zoom' | 'clips' | 'background' | 'cursor' | 'motion' | 'audio' | 'export';

export interface PropertiesPanelProps {
  onShowMouseOverlay?: (show: boolean) => void;
  showMouseOverlay?: boolean;
  isExporting?: boolean;
}

export interface TabProps {
  isExporting?: boolean;
}

export interface CursorTabProps extends TabProps {
  onShowMouseOverlay?: (show: boolean) => void;
  showMouseOverlay?: boolean;
}
