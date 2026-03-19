import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, CheckCircle, ExternalLink, RefreshCw } from 'lucide-react';
import { cn } from '../utils/cn';

interface PermissionStatus {
  accessibility_granted: boolean;
  screen_recording_granted: boolean;
  can_request_accessibility: boolean;
}

interface PermissionStatusProps {
  onPermissionsChanged?: (status: PermissionStatus) => void;
  className?: string;
}

const PermissionStatusComponent: React.FC<PermissionStatusProps> = ({
  onPermissionsChanged,
  className
}) => {
  const [permissionStatus, setPermissionStatus] = useState<PermissionStatus | null>(null);
  const [isRequesting, setIsRequesting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const checkPermissions = async () => {
    try {
      setIsRefreshing(true);
      const status = await invoke<PermissionStatus>('check_permissions');
      setPermissionStatus(status);
      onPermissionsChanged?.(status);
    } catch (error) {
      console.error('Failed to check permissions:', error);
    } finally {
      setIsRefreshing(false);
    }
  };

  const requestAccessibilityPermission = async () => {
    if (!permissionStatus?.can_request_accessibility) return;
    
    try {
      setIsRequesting(true);
      const granted = await invoke<boolean>('request_accessibility_permission');
      if (granted) {
        // Recheck permissions after a short delay
        setTimeout(checkPermissions, 1000);
      }
    } catch (error) {
      console.error('Failed to request accessibility permission:', error);
    } finally {
      setIsRequesting(false);
    }
  };

  const openAccessibilityPreferences = async () => {
    try {
      await invoke('open_accessibility_preferences');
      // Recheck permissions after user potentially grants them
      setTimeout(checkPermissions, 2000);
    } catch (error) {
      console.error('Failed to open accessibility preferences:', error);
    }
  };

  useEffect(() => {
    checkPermissions();
    
    // Recheck permissions periodically when they're not granted
    const interval = setInterval(() => {
      if (permissionStatus && (!permissionStatus.accessibility_granted || !permissionStatus.screen_recording_granted)) {
        checkPermissions();
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [permissionStatus]);

  if (!permissionStatus) {
    return (
      <div className={cn("flex items-center gap-2 p-3 rounded-lg bg-gray-100 dark:bg-gray-800", className)}>
        <RefreshCw className="w-4 h-4 animate-spin" />
        <span className="text-sm text-gray-600 dark:text-gray-400">Checking permissions...</span>
      </div>
    );
  }

  const allGranted = permissionStatus.accessibility_granted && permissionStatus.screen_recording_granted;
  const needsAttention = !allGranted;

  return (
    <div className={cn(
      "p-3 rounded-lg border transition-colors",
      needsAttention 
        ? "bg-amber-50 border-amber-200 dark:bg-amber-950/20 dark:border-amber-800" 
        : "bg-green-50 border-green-200 dark:bg-green-950/20 dark:border-green-800",
      className
    )}>
      <div className="flex items-start gap-3">
        {allGranted ? (
          <CheckCircle className="w-5 h-5 text-green-600 dark:text-green-400 flex-shrink-0 mt-0.5" />
        ) : (
          <AlertTriangle className="w-5 h-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
        )}
        
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-2">
            <h3 className={cn(
              "font-medium text-sm",
              allGranted 
                ? "text-green-800 dark:text-green-200" 
                : "text-amber-800 dark:text-amber-200"
            )}>
              {allGranted ? "Permissions Ready" : "Permissions Required"}
            </h3>
            
            <button
              onClick={checkPermissions}
              disabled={isRefreshing}
              className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
              title="Refresh permission status"
            >
              <RefreshCw className={cn("w-3 h-3", isRefreshing && "animate-spin")} />
            </button>
          </div>
          
          <div className="space-y-1 text-sm">
            <div className="flex items-center gap-2">
              {permissionStatus.accessibility_granted ? (
                <CheckCircle className="w-3 h-3 text-green-600 dark:text-green-400" />
              ) : (
                <AlertTriangle className="w-3 h-3 text-amber-600 dark:text-amber-400" />
              )}
              <span className={cn(
                permissionStatus.accessibility_granted 
                  ? "text-green-700 dark:text-green-300" 
                  : "text-amber-700 dark:text-amber-300"
              )}>
                Accessibility: {permissionStatus.accessibility_granted ? "Granted" : "Required for mouse tracking"}
              </span>
            </div>
            
            <div className="flex items-center gap-2">
              {permissionStatus.screen_recording_granted ? (
                <CheckCircle className="w-3 h-3 text-green-600 dark:text-green-400" />
              ) : (
                <AlertTriangle className="w-3 h-3 text-amber-600 dark:text-amber-400" />
              )}
              <span className={cn(
                permissionStatus.screen_recording_granted 
                  ? "text-green-700 dark:text-green-300" 
                  : "text-amber-700 dark:text-amber-300"
              )}>
                Screen Recording: {permissionStatus.screen_recording_granted ? "Granted" : "Required for capture"}
              </span>
            </div>
          </div>
          
          {needsAttention && (
            <div className="mt-3 flex flex-wrap gap-2">
              {!permissionStatus.accessibility_granted && permissionStatus.can_request_accessibility && (
                <button
                  onClick={requestAccessibilityPermission}
                  disabled={isRequesting}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-amber-800 dark:text-amber-200 bg-amber-100 dark:bg-amber-900/30 rounded-md hover:bg-amber-200 dark:hover:bg-amber-900/50 transition-colors disabled:opacity-50"
                >
                  {isRequesting ? (
                    <RefreshCw className="w-3 h-3 animate-spin" />
                  ) : (
                    <CheckCircle className="w-3 h-3" />
                  )}
                  Request Access
                </button>
              )}
              
              <button
                onClick={openAccessibilityPreferences}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-amber-800 dark:text-amber-200 bg-amber-100 dark:bg-amber-900/30 rounded-md hover:bg-amber-200 dark:hover:bg-amber-900/50 transition-colors"
              >
                <ExternalLink className="w-3 h-3" />
                Open Preferences
              </button>
            </div>
          )}
          
          {allGranted && (
            <p className="mt-2 text-xs text-green-600 dark:text-green-400">
              All permissions granted. Auto-zoom from mouse tracking is available.
            </p>
          )}
        </div>
      </div>
    </div>
  );
};

export default PermissionStatusComponent;