import { create } from 'zustand';

export type RecordingState = 'idle' | 'prerecord' | 'recording' | 'paused' | 'review';

interface RecordingStore {
  state: RecordingState;
  startTime: number | null;
  duration: number;
  filePath: string | null;
  setRecordingState: (state: RecordingState) => void;
  startRecording: () => void;
  stopRecording: (filePath: string) => Promise<void>;
  pauseRecording: () => void;
  resumeRecording: () => void;
}

export const useRecordingStore = create<RecordingStore>((set) => ({
  state: 'idle',
  startTime: null,
  duration: 0,
  filePath: null,
  
  setRecordingState: (state) => set({ state }),
  
  startRecording: () => set({ 
    state: 'recording', 
    startTime: Date.now(),
    duration: 0 
  }),
  
  stopRecording: async (filePath) => {
    console.log('Recording store: stopRecording called with filePath:', filePath);
    
    // Get actual video duration from FFmpeg
    let videoDuration = 0;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const videoInfo = await invoke('get_video_metadata', { filePath });
      videoDuration = (videoInfo as any).duration_ms;
      console.log('Recording store: got video duration from FFmpeg:', videoDuration, 'ms');
    } catch (error) {
      console.error('Recording store: failed to get video metadata, using estimated duration:', error);
      // Fallback to estimated duration using passed state
      const storeState = useRecordingStore.getState();
      videoDuration = storeState.startTime ? Date.now() - storeState.startTime : 30000;
    }
    
    set((state) => {
      console.log('Recording store: transitioning from', state.state, 'to review with duration:', videoDuration);
      return {
        state: 'review',
        filePath,
        duration: videoDuration
      };
    });
  },
  
  pauseRecording: () => set({ state: 'paused' }),
  
  resumeRecording: () => set({ state: 'recording' })
}));