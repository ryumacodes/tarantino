export interface BackgroundAudio {
  path: string;
  name: string;
  volume: number;
  muted: boolean;
}
export function exportBackgroundAudio(audio?: BackgroundAudio) {
  return audio ? {path:audio.path,volume:audio.volume,muted:audio.muted} : null;
}
/** Music follows edited playback time, independently of source cuts and video rate. */
export function createBackgroundAudioPlayer(audio: HTMLAudioElement, report: (error: unknown)=>void) {
  let disposed=false, pending=false, blocked=false, wantsPlaying=false;
  const failed=()=>{
    if (disposed || audio.error?.code===1) return;
    blocked=true;
    report(new Error('Background music could not be loaded. Replace the file in Audio settings if it was moved or is unsupported.'));
  };
  const started=()=>{if(!disposed)report(null);};
  audio.addEventListener('error',failed);
  audio.addEventListener('playing',started);
  audio.loop=true;
  audio.playbackRate=1;
  return {
    sync(timeMs: number, playing: boolean, settings: BackgroundAudio, retry=false) {
      if (disposed) return;
      wantsPlaying=playing;
      audio.volume=Math.max(0,Math.min(1,settings.volume));
      audio.muted=settings.muted;
      if (retry) blocked=false;
      const duration=audio.duration;
      if (Number.isFinite(duration) && duration>0) {
        const target=Math.max(0,timeMs/1000)%duration;
        const distance=Math.abs(audio.currentTime-target);
        const circularDistance=Math.min(distance,Math.abs(duration-distance));
        if ((!playing && distance>0.001) || circularDistance>0.08) audio.currentTime=target;
      }
      if (!playing) { audio.pause(); return; }
      if (audio.readyState<1 || !audio.paused || pending || blocked) return;
      pending=true;
      void audio.play().catch(error=>{
        if (!disposed) {blocked=true;report(error);}
      }).finally(()=>{pending=false;if(disposed || !wantsPlaying)audio.pause();});
    },
    retry() {
      if(disposed)return;
      blocked=false;
      if(audio.error)audio.load();
    },
    dispose() {disposed=true;audio.removeEventListener('error',failed);audio.removeEventListener('playing',started);audio.pause();audio.removeAttribute('src');audio.load();},
  };
}
