import type { EditorState } from '../../stores/editor/types';

export function typingSpeedSuggestions(state: Pick<EditorState,
  'typingRanges' | 'dismissedTypingRanges' | 'tracks' | 'videoFilePath'>) {
  return state.tracks.filter(track => track.visible && !track.locked)
    .flatMap(track => track.clips.filter(clip => clip.enabled && !clip.locked
      && (!clip.sourceFilePath || clip.sourceFilePath === state.videoFilePath)
      && (clip.type === 'video' || clip.type === 'screen')
      && Number.isFinite(clip.playbackRate) && clip.playbackRate > 0)
      .flatMap(clip => state.typingRanges
        .filter(range => !state.dismissedTypingRanges.includes(range.start))
        .map(range => ({clip: {...clip}, rangeStart: range.start,
          from: Math.max(clip.sourceIn, range.start), to: Math.min(clip.sourceOut, range.end)}))
        .filter(range => range.to-range.from >= 1500)));
}
