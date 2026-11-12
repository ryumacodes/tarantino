export const cursorSegmentsShader = `
vec2 cursorSegmentSplits(vec3 p) {
  if(abs(p.x)>0.00001*(abs(p.y)+abs(p.z)+0.000001)) {
    float discriminant=p.y*p.y-4.0*p.x*p.z;
    if(discriminant<=0.0)return vec2(1.0);
    float q=-0.5*(p.y+(p.y>=0.0?1.0:-1.0)*sqrt(discriminant));
    vec2 roots=vec2(q/p.x,p.z/q);
    return clamp(vec2(min(roots.x,roots.y),max(roots.x,roots.y)),0.0,1.0);
  }
  if(abs(p.y)>0.00000001)return vec2(clamp(-p.z/p.y,0.0,1.0),1.0);
  return vec2(1.0);
}
float cursorCombPhase(vec2 pixel) {
  uvec2 p=uvec2(pixel)%uvec2(8u);
  uint value=0u;
  for(uint bit=0u;bit<3u;bit++) {
    uint x=(p.x>>bit)&1u,y=(p.y>>bit)&1u;
    value+=((x^y)*2u+y)<<(4u-bit*2u);
  }
  return float(value)/64.0;
}
vec2 cursorSegmentLocal(vec2 pixel,vec4 pose) {
  return rotate2D(pixel-pose.xy,-pose.w*0.01745329252)/pose.z;
}
float cursorSegmentDistance(vec2 pixel,vec4 pose,vec4 rect) {
  vec2 delta=(abs(cursorSegmentLocal(pixel,pose)-rect.xy-rect.zw*0.5)-rect.zw*0.5)*pose.z;
  return length(max(delta,vec2(0.0)))+min(max(delta.x,delta.y),0.0);
}
vec4 cursorSegments(vec2 pixel,vec4 before,vec4 now) {
  vec4 rect=uCursorImageRect.z>0.0?uCursorImageRect:vec4(-24.0,-24.0,64.0,64.0);
  vec3 fit=motionFitDistance(cursorSegmentDistance(pixel,before,rect),
    cursorSegmentDistance(pixel,mix(before,now,0.5),rect),cursorSegmentDistance(pixel,now,rect));
  vec3 coverage=motionCoverageRamp(fit,0.0,1.5);
  vec2 splits=cursorSegmentSplits(fit);
  float edges[4]=float[4](0.0,splits.x,splits.y,1.0);
  vec2 dimensions=uCursorImageRect.z>0.0?uCursorImageSize.xy:vec2(256.0);
  vec2 footprint=dimensions/(rect.zw*before.z);
  float texelsPerPixel=max(footprint.x,footprint.y);
  float lod=max(0.0,log2(texelsPerPixel));
  float phase=cursorCombPhase(pixel);
  vec4 total=vec4(0.0);float weight=0.0;
  for(int segment=0;segment<3;segment++) {
    float span=edges[segment+1]-edges[segment];
    if(span<=0.0001)continue;
    weight+=span;
    float alpha=clamp(motionIntegrateCoverage(coverage,edges[segment],edges[segment+1])/span,0.0,1.0);
    if(alpha<=0.001)continue;
    vec2 start=(cursorSegmentLocal(pixel,mix(before,now,edges[segment]))-rect.xy)/rect.zw;
    vec2 end=(cursorSegmentLocal(pixel,mix(before,now,edges[segment+1]))-rect.xy)/rect.zw;
    vec2 delta=end-start,texelDelta=delta*dimensions;
    int count=int(clamp(ceil(length(texelDelta)/texelsPerPixel/uCursorFetchSpan),1.0,uCursorFetchBudget));
    vec2 along=delta/float(count);
    vec2 across=vec2(-texelDelta.y,texelDelta.x)/max(length(texelDelta),0.000001)*exp2(lod)/dimensions;
    vec4 color=vec4(0.0);
    for(int i=0;i<16;i++) {
      if(i>=count)break;
      vec2 uv=start+delta*(float(i)+phase)/float(count);
      vec4 sampled;
      if(uCursorFadeBlur<=0.04) {
        sampled=textureGrad(uCursorTile,vec2(uv.x,1.0-uv.y),along*vec2(1.0,-1.0),across*vec2(1.0,-1.0));
      } else {
        float blurLod=max(0.0,log2(max(min(length(along*dimensions),length(across*dimensions)),0.000001)));
        vec2 noise=vec2(texture2D(uCursorNoise,(mod(floor(pixel),32.0)+0.5)/32.0).r,
          texture2D(uCursorNoise,(mod(floor(pixel+vec2(37.0)),32.0)+0.5)/32.0).r);
        sampled=blurPyramid(uCursorTile,dimensions,uCursorImageRect.z>0.0?uCursorImageSize.z:9.0,uv,
          uCursorFadeBlur*dimensions.y/(rect.w*now.z),blurLod,noise,true);
      }
      vec3 rgb=clamp(sampled.rgb,vec3(0.0),vec3(sampled.a));
      color+=vec4(rgb*rgb/max(sampled.a,0.00001),sampled.a)/float(count);
    }
    total+=color*(alpha*span);
  }
  total/=weight;
  if(total.a<=0.0)return vec4(0.0);
  return vec4(sqrt(max(total.rgb/total.a,vec3(0.0))),total.a*uCursorOpacity);
}
`;
