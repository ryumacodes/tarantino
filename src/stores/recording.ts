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

interface VideoMetadata {
  duration_ms: number;
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
    let videoDuration = 0;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const videoInfo = await invoke<VideoMetadata>('get_video_metadata', { filePath });
      videoDuration = videoInfo.duration_ms;
    } catch (error) {
      console.error('Recording store: failed to get video metadata, using estimated duration:', error);
      const storeState = useRecordingStore.getState();
      videoDuration = storeState.startTime ? Date.now() - storeState.startTime : 30000;
    }
    
    set({
      state: 'review',
      filePath,
      duration: videoDuration
    });
  },
  
  pauseRecording: () => set({ state: 'paused' }),
  
  resumeRecording: () => set({ state: 'recording' })
}));
