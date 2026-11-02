/** Analytic shutter coverage shared in behavior with the native window renderer. */
export const temporalCoverageShader = `
vec3 motionFitDistance(float start,float middle,float end) {
  return vec3(2.0*start-4.0*middle+2.0*end,-3.0*start+4.0*middle-end,start);
}
vec3 motionCoverageRamp(vec3 distance,float inset,float antialias) {
  float inverseWidth=1.0/max(2.0*antialias,0.000001);
  return vec3(-distance.xy*inverseWidth,0.5-(distance.z-inset)*inverseWidth);
}
vec2 motionLevelRoots(vec3 p,float level,float lo,float hi) {
  float a=p.x,b=p.y,c=p.z-level;
  if(abs(a)<0.0000001) {
    if(abs(b)<0.0000001) return vec2(hi);
    return vec2(clamp(-c/b,lo,hi),hi);
  }
  float discriminant=b*b-4.0*a*c;
  if(discriminant<0.0) return vec2(hi);
  float signB=b>=0.0?1.0:-1.0;
  float q=-0.5*(b+signB*sqrt(discriminant));
  if(abs(q)<0.0000001) return vec2(clamp(-b/(2.0*a),lo,hi));
  return clamp(vec2(q/a,c/q),vec2(lo),vec2(hi));
}
float motionIntegrateCoverage(vec3 p,float start,float end) {
  if(end<=start) return 0.0;
  vec2 zero=motionLevelRoots(p,0.0,start,end),one=motionLevelRoots(p,1.0,start,end);
  float edges[6]=float[6](start,end,zero.x,zero.y,one.x,one.y);
  for(int i=1;i<6;i++) {
    for(int j=i;j>0;j--) {
      if(edges[j]>=edges[j-1]) break;
      float swap=edges[j];edges[j]=edges[j-1];edges[j-1]=swap;
    }
  }
  float area=0.0;
  for(int i=0;i<5;i++) {
    float span=edges[i+1]-edges[i],middle=(edges[i+1]+edges[i])*0.5;
    float value=(p.x*middle+p.y)*middle+p.z;
    if(value>=1.0) area+=span;
    else if(value>0.0) area+=clamp(span*(value+p.x*span*span/12.0),0.0,span);
  }
  return clamp(area,0.0,end-start);
}
`;
