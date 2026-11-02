export const loupePoolLighting = `
vec3 poolLight(float t) {
  float spectral=clamp(t,0.0,1.0),hue=(1.0-pow(spectral,0.6))*6.283185307179586;
  float ramp=smoothstep(0.0,0.4,spectral),amplitude=0.5*0.35*ramp;
  vec3 spectrum=vec3(cos(hue),cos(hue-2.0943951),cos(hue-4.1887902));
  vec3 warmth=vec3(1.0)+0.125*vec3(0.878,-0.023,-0.851);
  return (vec3(1.0)+amplitude*spectrum)*warmth*mix(0.5,1.0,ramp);
}
vec4 shadePool(vec4 content,vec2 relative,float coverage) {
  float radius=lensRadius*resolution.x,sourceScale=lensSourcePixelScale*resolution.x;
  float poolPresence=min(lensBevel*resolution.x/sourceScale*0.5,1.0)*lensPresence*lensLighting;
  vec3 pool=vec3(0.0);
  if(poolPresence>0.001) {
    if(coverage>0.001) {
      float innerU=clamp((length(relative-lensLightDirection*radius)-radius)/radius,0.0,1.0);
      pool+=poolLight(innerU)*innerU*innerU*0.25*poolPresence*coverage;
    }
    if(coverage<1.0) {
      float shift=min(radius*0.5,max(lensShadowReach*resolution.x,sourceScale)*0.45);
      float outerU=clamp((radius-length(relative+lensLightDirection*shift))/shift,0.0,1.0);
      pool+=poolLight(1.0-outerU)*outerU*outerU*0.3*poolPresence*(1.0-coverage);
    }
  }
  float shadow=outerShadow(relative,radius)*(1.0-coverage);
  vec3 light=pool*vec3(1.0,0.98,0.95),sheen=light*0.35;
  vec3 lit=content.rgb*(vec3(1.0-shadow)+light-sheen)+sheen;
  return vec4(lit/max(max(lit.r,max(lit.g,lit.b)),1.0),
    min(content.a+(1.0-content.a)*shadow+max(sheen.r,max(sheen.g,sheen.b)),1.0));
}
`;
