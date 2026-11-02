import {createScalarStepper} from '../motion/scalarSpring';
export {createScalarSpring as createKeycapSpring} from '../motion/scalarSpring';
export type {ScalarSpring as KeycapSpring} from '../motion/scalarSpring';
/** Shortcut keycaps retain their independent response preset. */
export const advanceKeycapSpring=createScalarStepper(300,2,1.5);
