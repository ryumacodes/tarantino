import {describe,expect,it} from 'vitest';
import {advanceKeycapSpring,createKeycapSpring} from './keycapSpring';
describe('keycap spring',()=>{
  it('settles exactly after entrance and reversal without losing momentum on retarget',()=>{
    let state=createKeycapSpring(0);
    for(let i=0;i<12;i++)state=advanceKeycapSpring(state,1,1000/60);
    expect(state.value).toBeGreaterThan(0);
    const reversed=advanceKeycapSpring(state,0,0);
    expect(reversed.velocity).toBe(state.velocity);
    expect(reversed.value).toBe(state.value);
    state=reversed;
    for(let i=0;i<600;i++)state=advanceKeycapSpring(state,0,1000/60);
    expect(state.value).toBe(0);expect(state.velocity).toBe(0);
  });
  it('keeps target-change precision fixed between steps and does not mutate input',()=>{
    const initial=createKeycapSpring(0),next=advanceKeycapSpring(initial,100,16);
    expect(initial).toEqual(createKeycapSpring(0));
    expect(next.precision).toBe(.2);
    expect(advanceKeycapSpring(next,100,16).precision).toBe(.2);
  });
  it('rejects non-finite or unbounded simulation requests',()=>{
    for(const ms of [-1,Infinity,NaN,10001])
      expect(()=>advanceKeycapSpring(createKeycapSpring(0),1,ms)).toThrow(RangeError);
  });
});
