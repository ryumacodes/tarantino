export const loupeLighting = `
uniform float lensLighting;
uniform float lensShineExponent;
vec4 composeRimLight(vec4 content,vec3 light) {
  vec3 sheen=light*0.55;
  vec3 lit=content.rgb*(vec3(1.0)+light*0.9-sheen)+sheen;
  return vec4(lit/max(max(lit.r,max(lit.g,lit.b)),1.0),min(content.a+max(sheen.r,max(sheen.g,sheen.b)),1.0));
}
vec2 highlightArcs(float facing,float width,float edge,float softness) {
  float start=1.0-width;
  float feather=max(mix(width,0.02,edge)*(1.0+softness*0.5),0.01);
  return vec2(smoothstep(start,start+feather,max(facing,0.0)),
    0.28*smoothstep(start+width*0.25,start+feather+width*0.25,max(-facing,0.0)));
}
vec4 shadeRim(vec4 content,vec2 relative,float coverage,vec2 pixel) {
  float distance=length(relative),radius=lensRadius*resolution.x;
  float bevel=lensBevel*resolution.x,sourceScale=lensSourcePixelScale*resolution.x;
  if(lensLighting<=0.0 || bevel<=0.0 || coverage<=0.0 || distance>=radius || distance<0.001*sourceScale) return content;
  float bevelBand=max(bevel,sourceScale);
  float shineBand=max(radius*0.036*pow(3.0,(0.5-0.9)*2.0),sourceScale);
  float innerWidth=shineBand*50.0;
  if(distance<radius-max(bevelBand,innerWidth)) return content;
  float t=clamp((distance-radius+bevelBand)/bevelBand,0.0,1.0);
  float bevelPresence=min(bevel/sourceScale*0.5,1.0)*lensLighting;
  float facing=dot(relative/distance,lensLightDirection),edgeDistance=radius-distance;
  float core=pow(clamp(1.0-edgeDistance/shineBand,0.0,1.0),2.5);
  float innerU=clamp(1.0-edgeDistance/innerWidth,0.0,1.0);
  float inner=innerU*innerU*(3.0-2.0*innerU)*0.1;
  float softness=clamp(4.0/max(lensShineExponent,1.0),0.0,1.0);
  vec2 arcs=highlightArcs(facing,0.3,0.25,softness)*core+highlightArcs(facing,0.55,0.125,softness)*inner;
  vec3 tint=vec3(1.0,0.98,0.95);
  vec3 light=(tint*arcs.x+tint*vec3(0.9,0.96,1.05)*arcs.y+tint*0.05*(core+inner))
    *bevelPresence*coverage*lensPresence*1.05*(1.0+(orderedThreshold(pixel)-0.5)*0.8);
  float darken=0.14*t*t*bevelPresence*coverage;
  return composeRimLight(vec4(content.rgb*(1.0-darken),content.a),light);
}
`;
