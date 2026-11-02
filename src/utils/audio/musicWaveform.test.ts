import { expect,it } from 'vitest';
import { musicPeak, musicPeakRange } from './musicWaveform';
it('loops using exact decoded duration including the partial final bin',()=>{
 const peaks=[0.2,0.5,0.8];
 expect([0,10,24,25,35,49,50,60].map(t=>musicPeak(peaks,25,t))).toEqual([0.2,0.5,0.8,0.2,0.5,0.8,0.2,0.5]);
 expect(musicPeak(peaks,25,25010)).toBe(0.5);
 expect(musicPeak([],0,100)).toBe(0);
});

it('preserves brief peaks at low zoom and across a partial-bin loop boundary',()=>{
 expect(musicPeakRange([0.2,0.9,0.1],25,0,25,0.9)).toBe(0.9);
 expect(musicPeakRange([0.8,0.1,0.9],25,24,26,0.9)).toBe(0.9);
 expect(musicPeakRange([0.8,0.1,0.9],25,10,20,0.9)).toBe(0.1);
 expect(musicPeakRange([0.2,0.9,0.1],25,0,50000,0.9)).toBe(0.9);
});
