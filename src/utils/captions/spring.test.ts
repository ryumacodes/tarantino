import {expect,it} from 'vitest';
import {advanceCaptionSpring,createCaptionSpring} from './spring';
import {advanceKeycapSpring,createKeycapSpring} from '../shortcuts/keycapSpring';
import {createScalarStepper} from '../motion/scalarSpring';
it('retains velocity on reversal and settles exactly after caption disappearance',()=>{
 let state=createCaptionSpring(0);
 for(let i=0;i<8;i++)state=advanceCaptionSpring(state,1,1000/60);
 const reversed=advanceCaptionSpring(state,0,0);
 expect(reversed.velocity).toBe(state.velocity);expect(reversed.value).toBe(state.value);
 state=reversed;
 for(let i=0;i<300;i++)state=advanceCaptionSpring(state,0,1000/60);
 expect(state.value).toBe(0);expect(state.velocity).toBe(0);
});
it('uses separate caption dynamics without mutating shared input state',()=>{
 const initial=createCaptionSpring(0);
 const caption=advanceCaptionSpring(initial,40,1000/60);
 const shortcut=advanceKeycapSpring(createKeycapSpring(0),40,1000/60);
 expect(caption.value).not.toBe(shortcut.value);expect(initial).toEqual(createCaptionSpring(0));
});
it('validates dynamics once before a stepper can be used',()=>{
 for(const [stiffness,damping,mass] of [[NaN,12,1.5],[425,-1,1.5],[425,12,0],[0,12,1.5]])
  expect(()=>createScalarStepper(stiffness,damping,mass)).toThrow(RangeError);
});
