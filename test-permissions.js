// Simple test script to check permissions via Tauri commands
// This can be run in the browser console when the app is loaded

async function testScreenRecordingPermissions() {
    try {
        console.log('=== Testing Screen Recording Permissions ===');
        
        // Check current permissions
        const permissions = await window.__TAURI__.core.invoke('check_permissions');
        console.log('Current permissions:', permissions);
        
        // Diagnose screen capture
        const diagnosis = await window.__TAURI__.core.invoke('diagnose_screen_capture');
        console.log('Screen capture diagnosis:\n', diagnosis);
        
        // Try to request screen recording permission
        if (!permissions.screen_recording_granted) {
            console.log('Requesting screen recording permission...');
            const granted = await window.__TAURI__.core.invoke('request_screen_recording_permission');
            console.log('Permission granted:', granted);
        }
        
        return permissions;
    } catch (error) {
        console.error('Permission test failed:', error);
        throw error;
    }
}

// Export for easy use
window.testScreenRecordingPermissions = testScreenRecordingPermissions;

// Auto-run when the script loads
testScreenRecordingPermissions().then(permissions => {
    if (!permissions.screen_recording_granted) {
        console.warn('⚠️ Screen recording permission not granted. Recording may fail.');
        console.warn('Please run: window.__TAURI__.core.invoke("open_screen_recording_preferences")');
    } else {
        console.log('✅ All permissions granted!');
    }
});