/** Scalar spring state shared by animated overlay properties. */
export interface ScalarSpring {
  value:number;
  velocity:number;
  target:number;
  precision:number;
}

export function createScalarSpring(value:number):ScalarSpring {
  if(!Number.isFinite(value))throw new RangeError('Invalid spring value');
  return {value,velocity:0,target:value,precision:1};
}

/** Validate fixed dynamics once, then preserve velocity and precision on retargets. */
export function createScalarStepper(stiffness:number,baseDamping:number,mass:number) {
  if(![stiffness,baseDamping,mass].every(Number.isFinite)||stiffness<=0||baseDamping<0||mass<=0)
    throw new RangeError('Invalid spring dynamics');
  const brakingVelocity=Math.sqrt(stiffness/mass),criticalDamping=2*Math.sqrt(stiffness*mass);
  return (state:ScalarSpring,target:number,elapsedMs:number):ScalarSpring=>{
    if(!Number.isFinite(target)||!Number.isFinite(elapsedMs)||elapsedMs<0||elapsedMs>10_000)
      throw new RangeError('Invalid spring animation step');
    if(state.target===target&&state.value===target&&state.velocity===0)return state;
    let {value,velocity,precision}=state;
    if(target!==state.target)precision=value===target?1:Math.max(1,Math.abs(value-target))/500;
    if(elapsedMs===0||(value===target&&velocity===0))return {value,velocity,target,precision};
    const distance=target-value;
    let damping=baseDamping;
    if(velocity*distance>0&&Math.abs(velocity)>Math.abs(distance)*brakingVelocity)
      damping=criticalDamping;
    const steps=Math.ceil(elapsedMs);
    for(let step=1;step<=steps;step++) {
      // Fractional cadence uses the complement of the last millisecond.
      // Keep this convention stable: it affects sampled motion at 24/60/120 Hz.
      const seconds=step>elapsedMs?(step-elapsedMs)/1000:0.001;
      velocity+=(-stiffness*(value-target)-damping*velocity)/mass*seconds;
      value+=velocity*seconds;
      if(Math.abs(velocity)<precision&&Math.abs(value-target)<precision) {
        value=target;velocity=0;break;
      }
    }
    return {value,velocity,target,precision};
  };
}
