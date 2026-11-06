import { temporalCoverageShader } from './temporalCoverage.glsl';

export const temporalWindowShader = `
precision highp float;
precision highp int;
uniform sampler2D sourceTexture, maskTexture, shadowTexture;
uniform mat3 sourceTransform, maskTransform;
uniform vec2 outputSize, inputSize, baseCenter, sourceSize;
uniform vec4 shutterStart, shutterNow;
uniform float cornerRadius, hasNativeMask, shadowEnabled, fixedDisplay;
out vec4 outputColor;
${temporalCoverageShader}
float windowDistance(vec2 pixel,vec4 rect) {
  float fallback=hasNativeMask>0.5?0.0:min(inputSize.x,inputSize.y)*0.022;
  float radius=min(max(cornerRadius,fallback)*rect.z/inputSize.x,min(rect.z,rect.w)*0.5);
  vec2 q=abs(pixel-rect.xy)-rect.zw*0.5+vec2(radius);
  return length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-radius;
}
vec4 windowShadow(vec2 pixel,vec4 rect) {
  vec2 local=(pixel-rect.xy)/rect.zw*inputSize+baseCenter;
  vec2 uv=local/outputSize;
  if(any(lessThan(uv,vec2(0.0)))||any(greaterThan(uv,vec2(1.0)))) return vec4(0.0);
  vec4 shadow=texture(shadowTexture,vec2(uv.x,1.0-uv.y));
  return vec4(shadow.rgb*shadow.rgb*shadow.a,shadow.a);
}
vec4 displayContent(vec2 pixel) {
  vec2 local=pixel-baseCenter+inputSize*0.5;
  vec4 content=vec4(0.0);
  if(all(greaterThanEqual(local,vec2(0.0)))&&all(lessThan(local,inputSize))) {
    vec2 a=(pixel-shutterStart.xy)/shutterStart.zw+vec2(0.5);
    vec2 b=(pixel-shutterNow.xy)/shutterNow.zw+vec2(0.5);
    int count=int(clamp(ceil(length((b-a)*sourceSize)/4.0),1.0,16.0));
    for(int tap=0;tap<16;tap++) {
      if(tap>=count) break;
      vec2 uv=clamp(mix(a,b,(float(tap)+0.5)/float(count)),vec2(0.0),vec2(1.0));
      vec4 value=texture(sourceTexture,(sourceTransform*vec3(uv.x,1.0-uv.y,1.0)).xy);
      content+=vec4(value.rgb*value.rgb*value.a,value.a)/float(count);
    }
    float radius=min(cornerRadius,min(inputSize.x,inputSize.y)*0.5);
    vec2 q=abs(pixel-baseCenter)-inputSize*0.5+vec2(radius);
    float distance=length(max(q,vec2(0.0)))-radius;
    float alpha=radius>0.0?1.0-smoothstep(-0.5,0.5,distance):1.0;
    vec2 maskUv=local/inputSize;
    alpha*=texture(maskTexture,(maskTransform*vec3(maskUv.x,1.0-maskUv.y,1.0)).xy).r;
    content*=alpha;
  }
  if(content.a>0.0)content.rgb=sqrt(max(content.rgb/content.a,vec3(0.0)))*content.a;
  if(shadowEnabled>0.5) {
    vec2 uv=pixel/outputSize;
    vec4 shadow=texture(shadowTexture,vec2(uv.x,1.0-uv.y));
    content+=vec4(shadow.rgb*shadow.a,shadow.a)*(1.0-content.a);
  }
  return content;
}
void main() {
  // Native composition uses integer top-left pixel positions.
  vec2 pixel=vec2(floor(gl_FragCoord.x),outputSize.y-1.0-floor(gl_FragCoord.y));
  if(fixedDisplay>0.5){outputColor=displayContent(pixel);return;}
  vec4 before=shutterStart,now=shutterNow;
  vec2 low=min(before.xy-before.zw*0.5,now.xy-now.zw*0.5)-vec2(1.0);
  vec2 high=max(before.xy+before.zw*0.5,now.xy+now.zw*0.5)+vec2(1.0);
  if(shadowEnabled>0.5) {
    vec2 startUv=-baseCenter/inputSize,endUv=(outputSize-baseCenter)/inputSize;
    low=min(low,min(before.xy+startUv*before.zw,now.xy+startUv*now.zw));
    high=max(high,max(before.xy+endUv*before.zw,now.xy+endUv*now.zw));
  }
  outputColor=vec4(0.0);
  if(any(lessThan(pixel,low))||any(greaterThan(pixel,high))) return;
  vec3 distance=motionFitDistance(windowDistance(pixel,before),windowDistance(pixel,(before+now)*0.5),windowDistance(pixel,now));
  vec3 coverage=motionCoverageRamp(distance,0.0,0.5);
  vec2 roots=motionLevelRoots(distance,0.0,0.0,1.0);
  float edges[4]=float[4](0.0,min(roots.x,roots.y),max(roots.x,roots.y),1.0);
  vec2 uvStart=(pixel-before.xy)/before.zw+vec2(0.5),uvEnd=(pixel-now.xy)/now.zw+vec2(0.5);
  vec4 accumulated=vec4(0.0);float totalWeight=0.0;
  for(int segment=0;segment<3;segment++) {
    float start=edges[segment],end=edges[segment+1],span=end-start;
    if(span<0.0001) continue;
    float alpha=motionIntegrateCoverage(coverage,start,end)/span;
    vec4 content=vec4(0.0);
    if(alpha>0.001) {
      vec2 a=mix(uvStart,uvEnd,start),b=mix(uvStart,uvEnd,end);
      float travel=length((b-a)*sourceSize);
      int count=int(clamp(ceil(travel/4.0),1.0,16.0));
      for(int tap=0;tap<16;tap++) {
        if(tap>=count) break;
        vec2 uv=clamp(mix(a,b,(float(tap)+0.5)/float(count)),vec2(0.0),vec2(1.0));
        vec3 bottomUv=vec3(uv.x,1.0-uv.y,1.0);
        vec4 color=texture(sourceTexture,(sourceTransform*bottomUv).xy);
        float mask=texture(maskTexture,(maskTransform*bottomUv).xy).r;
        float opacity=color.a*mask;
        content+=vec4(color.rgb*color.rgb*opacity,opacity)/float(count);
      }
      content*=alpha;
    }
    if(shadowEnabled>0.5 && content.a<0.999) content+=windowShadow(pixel,mix(before,now,(start+end)*0.5))*(1.0-content.a);
    float weight=span*(1.0+2.0*smoothstep(0.0,1.0,1.0-content.a));
    accumulated+=content*weight;totalWeight+=weight;
  }
  if(totalWeight<=0.0 || accumulated.a<=0.0) return;
  float alpha=accumulated.a/totalWeight;
  outputColor=vec4(sqrt(max(accumulated.rgb/accumulated.a,vec3(0.0)))*alpha,alpha);
}
`;
