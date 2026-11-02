/** Three-tap variance-matched pyramid sampling, in top-left texture coordinates.
 * The caller supplies stable screen-space noise and premultiplied texture data. */
export const mipBlurShader = `
vec2 blurMipStretch(vec2 size,float level) {
  float span=exp2(level);
  return size/(max(floor(size/span),vec2(1.0))*span);
}
vec4 blurMipRead(sampler2D source,vec2 uv,float lod) {
  return textureLod(source,vec2(uv.x,1.0-uv.y),lod);
}
vec4 blurMipReadBorder(sampler2D source,vec2 size,vec2 uv,float lod) {
  vec2 dimensions=max(floor(size/exp2(lod)),vec2(1.0));
  vec2 p=uv*dimensions;
  vec2 coverage=clamp(min(p+vec2(0.5),dimensions+vec2(0.5)-p),vec2(0.0),vec2(1.0));
  return blurMipRead(source,uv,lod)*coverage.x*coverage.y;
}
vec4 blurMipSample(sampler2D source,vec2 size,vec2 uv,float lod,vec2 offset,bool transparent) {
  float level=floor(lod),fraction=fract(lod);
  vec2 s0=blurMipStretch(size,level),s1=blurMipStretch(size,level+1.0);
  vec2 position=uv+offset/size;
  if(transparent) {
    float upper=min(level+1.0,floor(log2(max(size.x,size.y))));
    return mix(blurMipReadBorder(source,size,position*s0,level),
      blurMipReadBorder(source,size,position*blurMipStretch(size,upper),upper),fraction);
  }
  if(any(greaterThan(abs(s1-s0),vec2(0.01)))) {
    return mix(blurMipRead(source,position*s0,level),blurMipRead(source,position*s1,level+1.0),fraction);
  }
  return blurMipRead(source,position*mix(s0,s1,fraction),lod);
}
vec4 blurPyramid(sampler2D source,vec2 size,float levels,vec2 uv,float radius,float minLod,vec2 noise,bool transparent) {
  float sigma=max(radius,0.0)*0.5;
  if(sigma<=0.02) {
    if(!transparent)return blurMipRead(source,uv,minLod);
    return blurMipSample(source,size,uv,minLod,vec2(0.0),true);
  }
  float spread=0.6*clamp(1.0-(log2(sigma)-1.0)/3.0,0.0,1.0);
  float ring=0.5+0.4*spread,jitter=1.6*spread;
  float variance=1.0/6.0+ring*ring*(1.0+jitter*jitter/12.0)*0.5;
  float spanSquared=(sigma*sigma+0.75)/(0.25+variance);
  if(spanSquared<4.0) spanSquared=(sigma*sigma+1.0/12.0)/(1.0/12.0+variance);
  float span=sqrt(spanSquared);
  float lod=clamp(max(log2(span),minLod),0.0,levels-1.0);
  float ringRadius=ring*span;
  if(ringRadius<0.05) return blurMipSample(source,size,uv,lod,vec2(0.0),transparent);
  float radiusPx=ringRadius*(1.0+jitter*(fract(noise.y)-0.5));
  vec4 result=vec4(0.0);
  for(int i=0;i<3;i++) {
    float angle=(noise.x+float(i)/3.0)*6.283185307179586;
    result+=blurMipSample(source,size,uv,lod,vec2(cos(angle),sin(angle))*radiusPx,transparent);
  }
  return result/3.0;
}
`;
