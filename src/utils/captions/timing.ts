/** Transcript timing is stored relative to the audio session, in milliseconds. */
export interface TimedTranscriptText {text:string;startMs:number;endMs:number}

/** Estimate spoken duration for correcting recognizer intervals containing long pauses. */
export function estimatedSpeechDuration(text:string):number {
  let syllables=0;
  for(const token of text.trim().split(/\s+/)) {
    const word=token.replace(/[^\w]/g,'').toLowerCase();
    if(!word)continue;
    if(word.length<=3){syllables++;continue;}
    const stem=word.replace(/(?:[^laeiouy]es|ed|[^laeiouy]e)$/,'').replace(/^y/,'');
    syllables+=stem.match(/[aeiouy]{1,2}/g)?.length??1;
  }
  return syllables/3*1000;
}

/** Preserve recognition/edit metadata while moving a session into source time.
 * Do not repeatedly normalize already-adjusted words when applying timeline edits. */
export function normalizeTranscriptTiming<T extends TimedTranscriptText>(words:readonly T[],sessionStartMs=0):T[] {
  if(!Number.isFinite(sessionStartMs)||sessionStartMs<0)throw new RangeError('Invalid transcript session start');
  return words.map(word=>{
    if(typeof word.text!=='string'||!Number.isFinite(word.startMs)||!Number.isFinite(word.endMs)
      ||word.startMs<0||word.endMs<word.startMs)throw new RangeError('Invalid transcript timing');
    const duration=estimatedSpeechDuration(word.text);
    const start=word.endMs-word.startMs>duration*2?word.endMs-duration:word.startMs;
    const startMs=start+sessionStartMs,endMs=word.endMs+sessionStartMs;
    if(!Number.isFinite(startMs)||!Number.isFinite(endMs))throw new RangeError('Transcript timing overflow');
    return {...word,startMs,endMs};
  });
}
