import {mipBlurShader} from './shaders/mipBlur.glsl';
export const loupeFiltering = `
uniform sampler2D lensPyramid;
uniform sampler2D lensForeground;
uniform sampler2D lensBackground;
uniform float lensSeparateLayers;
uniform sampler2D lensNoise;
uniform float lensMipCount;
float blueNoise(vec2 pixel) {
  return texture2D(lensNoise,(mod(floor(pixel),32.0)+0.5)/32.0).r;
}
${mipBlurShader}
vec4 filteredRead(vec2 uv,float radius,vec2 pixel) {
  if(max(radius,0.0)*0.5<=0.02)return texture2D(lensForeground,clamp(uv,vec2(0.0),vec2(1.0)));
  return blurPyramid(lensPyramid,resolution,lensMipCount,vec2(uv.x,1.0-uv.y),radius,0.0,
    vec2(blueNoise(pixel),blueNoise(pixel+vec2(37.0))),false);
}
vec2 refractedPosition(vec2 position) {
  vec2 relative=position-lensCenter*resolution;
  float distance=length(relative),radius=lensRadius*resolution.x;
  float bevel=lensBevel*resolution.x,sourceScale=lensSourcePixelScale*resolution.x;
  vec2 sampled=lensCenter*resolution+relative/lensZoom;
  float band=max(bevel,sourceScale);
  if(bevel>0.0 && distance>0.001*sourceScale && distance<radius && distance>radius-band) {
    float t=clamp((distance-radius+band)/band,0.0,1.0),squared=t*t;
    float slope=min(squared*inversesqrt(1.0-min(squared,0.99)),6.0);
    sampled-=relative/distance*slope*(1.0-1.0/1.5)*bevel/lensZoom;
  }
  return sampled;
}
vec2 fullRefractedPosition(vec2 position) {
  vec2 ordinary=lensCenter*resolution+(position-lensCenter*resolution)/lensZoom;
  return ordinary-(ordinary-refractedPosition(position))*lensZoom;
}
float rimBlurRadius(vec2 position,vec2 sampled) {
  vec2 relative=position-lensCenter*resolution;
  float distance=length(relative),sourceScale=lensSourcePixelScale*resolution.x;
  float radius=lensRadius*resolution.x,bevel=lensBevel*resolution.x;
  if(bevel<=0.0 || distance>=radius) return 0.0;
  float scale=1.0/max(lensZoom,0.05);
  if(distance>=sourceScale && distance>radius-max(bevel,sourceScale)) {
    vec2 fullSample=fullRefractedPosition(position);
    vec2 inward=fullRefractedPosition(position-relative/distance*sourceScale);
    float radial=length(fullSample-inward)/sourceScale;
    float tangential=length(fullSample-lensCenter*resolution)/distance;
    scale=max(radial,tangential);
  }
  float lod=clamp(clamp(log2(max(scale,0.01)),-2.0,4.0)+log2(max(lensZoom,0.05)),0.0,4.0);
  if(lod<0.05) return 0.0;
  return 0.58*(exp2(lod)-1.0)/lensZoom;
}
`;
