export type TypingRange = { start: number; end: number };

/** Keyboard-activity sections in recording time; no text or caret data required. */
export function typingRanges(input: unknown): TypingRange[] {
  if (!Array.isArray(input)) return [];
  const times = input.filter(event => !event?.shortcut).map(event => event?.timestamp)
    .filter((time): time is number => typeof time === 'number' && Number.isFinite(time) && time >= 0)
    .sort((a,b) => a-b);
  const ranges: TypingRange[] = [];
  let first = 0;
  const finish = (end: number) => {
    const duration = times[end]-times[first];
    if (duration >= 1500 && (end-first+1)*1000/duration >= 2) {
      ranges.push({start:times[first],end:times[end]});
    }
  };
  for (let i=1;i<times.length;i++) {
    if (times[i]-times[i-1] >= 500) { finish(i-1); first=i; }
  }
  if (times.length) finish(times.length-1);
  return ranges;
}
