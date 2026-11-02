import {createScalarStepper} from '../motion/scalarSpring';
export {createScalarSpring as createCaptionSpring} from '../motion/scalarSpring';
export type {ScalarSpring as CaptionSpring} from '../motion/scalarSpring';
/** Applied independently to caption box and word position, size and opacity. */
export const advanceCaptionSpring=createScalarStepper(425,12,1.5);
