import {expect,it} from 'vitest';
import {estimatedSpeechDuration,normalizeTranscriptTiming} from './timing';
it('offsets sessions and shortens long recognition intervals while preserving editable metadata',()=>{
 const input=[{text:'hello',startMs:0,endMs:3000,type:'word',speaker:'one'}];
 const result=normalizeTranscriptTiming(input,5000);
 expect(result[0].startMs).toBeCloseTo(8000-2000/3);
 expect(result[0].endMs).toBe(8000);expect(result[0].speaker).toBe('one');
 expect(input[0].startMs).toBe(0);
});
it('preserves ordinary durations and the exact long-pause threshold',()=>{
 expect(estimatedSpeechDuration('  cat  ')).toBe(1000/3);
 const word={text:'cat',startMs:100,endMs:100+2000/3};
 expect(normalizeTranscriptTiming([word])[0]).toEqual(word);
 expect(normalizeTranscriptTiming([{...word,endMs:400}])[0].startMs).toBe(100);
});
it('rejects corrupt input rather than creating invalid source ranges',()=>{
 for(const word of [{text:'a',startMs:NaN,endMs:10},{text:'a',startMs:20,endMs:10},
 {text:'a',startMs:-1,endMs:10},{text:'a',startMs:0,endMs:Infinity}])
  expect(()=>normalizeTranscriptTiming([word])).toThrow(RangeError);
 expect(()=>normalizeTranscriptTiming([],Infinity)).toThrow(RangeError);
});
