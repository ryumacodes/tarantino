import {temporalCoverageShader} from './temporalCoverage.glsl';
import {cursorSegmentsShader} from './cursorSegments.glsl';
import {mipBlurShader} from './mipBlur.glsl';
import {cursorBodyShader} from './cursorBody.glsl';
// SDF-based cursor rendering fragment shader for postprocessing
// A cached premultiplied tile supplies spatial blur during cursor fades.
// Matches Canvas 2D cursor overlay and export WGSL cursor rendering exactly.

export const cursorFragmentShader = `
uniform float uCursorTemporalEnabled;
uniform float uCursorFetchBudget,uCursorFetchSpan,uCursorStageHeightRatio;
uniform vec4 uCursorShutterStart,uCursorShutterNow;
uniform vec4 uCursorClipBefore,uCursorClipNow;
uniform float uCursorX;          // screen UV (0-1)
uniform float uCursorY;          // screen UV (0-1)
uniform float uCursorScale;      // visual scale multiplier
uniform sampler2D uCursorTile;
uniform vec4 uCursorImageRect;
uniform vec3 uCursorImageSize;
uniform sampler2D uCursorNoise;
uniform float uCursorFadeBlur;
uniform float uCursorOpacity;    // idle fade (0-1)
uniform float uCursorRotation;   // degrees
uniform float uCursorStyle;      // 0=pointer,1=circle,2=filled,3=outline,4=dotted
uniform float uIsClicking;       // 0 or 1
uniform float uClickEffect;     // 0=none,1=circle,2=ripple

// Colors (RGB, 0-1)
uniform float uCursorColorR;
uniform float uCursorColorG;
uniform float uCursorColorB;
uniform float uHighlightColorR;
uniform float uHighlightColorG;
uniform float uHighlightColorB;
uniform float uRippleColorR;
uniform float uRippleColorG;
uniform float uRippleColorB;

// Shadow
uniform float uShadowIntensity; // 0-100

// Ripple effect
uniform float uRippleProgress;  // 0=inactive, 0-1=active
uniform float uRippleX;         // screen UV
uniform float uRippleY;

// Circle highlight
uniform float uCircleHlProgress; // 0=inactive, 0-1=active
uniform float uCircleHlX;        // screen UV
uniform float uCircleHlY;

// Trail
uniform float uTrailEnabled;
uniform float uTrailCount;       // 0-30
uniform float uTrailOpacity;     // 0-1
uniform vec4 uTrailPoints[30];   // (x, y, alpha, size) in screen UV

// Resolution for pixel-space math
uniform float uResolutionX;
uniform float uResolutionY;

// Video plane bounds clipping (window mode only)
uniform float uVideoClipEnabled;
uniform float uVideoClipMinX;  // pixel coords
uniform float uVideoClipMinY;
uniform float uVideoClipMaxX;
uniform float uVideoClipMaxY;

${cursorBodyShader}
${mipBlurShader}
${temporalCoverageShader}
${cursorSegmentsShader}
vec4 sampledCursorBody(vec2 pixel,vec4 pose,float footprintSize,float fadeSize) {
  if(uCursorOpacity<=0.01 || pose.z<=0.0)return vec4(0.0);
  vec2 local=rotate2D(pixel-pose.xy,-pose.w*0.01745329252)/pose.z;
  vec4 rect=uCursorImageRect.z>0.0?uCursorImageRect:vec4(-24.0,-24.0,64.0,64.0);
  vec3 imageSize=uCursorImageRect.z>0.0?uCursorImageSize:vec3(256.0,256.0,9.0);
  vec2 edge=(abs(local-rect.xy-rect.zw*0.5)-rect.zw*0.5)*pose.z;
  float distance=length(max(edge,vec2(0.0)))+min(max(edge.x,edge.y),0.0);
  float coverage=1.0-smoothstep(-1.5,1.5,distance);
  if(coverage<=0.0)return vec4(0.0);
  vec2 uv=(local-rect.xy)/rect.zw;
  vec2 texelsPerPixel=imageSize.xy/(rect.zw*footprintSize);
  float samplingLod=max(0.0,log2(max(texelsPerPixel.x,texelsPerPixel.y)));
  vec4 sampled=vec4(0.0);
  if(uCursorFadeBlur<=0.04) {
    sampled=blurMipRead(uCursorTile,uv,samplingLod);
  } else {
    vec2 noise=vec2(texture2D(uCursorNoise,(mod(floor(pixel),32.0)+0.5)/32.0).r,
      texture2D(uCursorNoise,(mod(floor(pixel+vec2(37.0)),32.0)+0.5)/32.0).r);
    float texelScale=imageSize.y/(rect.w*fadeSize);
    sampled=blurPyramid(uCursorTile,imageSize.xy,imageSize.z,uv,uCursorFadeBlur*texelScale,samplingLod,noise,true);
  }
  return sampled.a>0.0 ? vec4(sampled.rgb/sampled.a,sampled.a*uCursorOpacity*coverage) : vec4(0.0);
}
float cursorBodyRadius() {
  if(uCursorImageRect.z<=0.0)return length(vec2(40.0));
  return length(max(abs(uCursorImageRect.xy),abs(uCursorImageRect.xy+uCursorImageRect.zw)));
}
bool cursorTemporalActive() {
  if(uCursorTemporalEnabled<=0.5 || uCursorOpacity<=0.01)return false;
  vec4 before=uCursorShutterStart,now=uCursorShutterNow;
  vec4 rect=uCursorImageRect.z>0.0?uCursorImageRect:vec4(-24.0,-24.0,64.0,64.0);
  vec4 oldBox=vec4(before.xy+rect.xy*before.z,rect.zw*before.z);
  vec4 newBox=vec4(now.xy+rect.xy*now.z,rect.zw*now.z);
  float tolerance=uResolutionY*uCursorStageHeightRatio/1080.0;
  float rotationTravel=abs(now.w-before.w)*0.01745329252*cursorBodyRadius()*max(before.z,now.z);
  return any(greaterThanEqual(abs(newBox-oldBox),vec4(tolerance)))
    || any(greaterThanEqual(abs(now.xy-before.xy),vec2(tolerance))) || rotationTravel>=tolerance;
}
vec4 temporalCursorBody(vec2 pixel) {
  vec4 before=uCursorShutterStart,now=uCursorShutterNow;
  float extent=cursorBodyRadius();
  float radius=extent*max(before.z,now.z);
  float travel=length(now.xy-before.xy)+abs(now.w-before.w)*0.01745329252*radius+extent*abs(now.z-before.z);
  int count=int(clamp(ceil(travel),2.0,64.0));
  if(uVideoClipEnabled<0.5) {
    vec2 startLocal=rotate2D(pixel-before.xy,-before.w*0.01745329252)/before.z;
    vec2 endLocal=rotate2D(pixel-now.xy,-now.w*0.01745329252)/now.z;
    if(length((endLocal-startLocal)*before.z)<1.0)
      return sampledCursorBody(pixel,mix(before,now,0.5),before.z,now.z);
    return cursorSegments(pixel,before,now);
  }
  vec4 accumulated=vec4(0.0);
  for(int i=0;i<64;i++) {
    if(i>=count)break;
    float t=(float(i)+0.5)/float(count);
    if(uVideoClipEnabled>0.5) {
      vec4 rect=mix(uCursorClipBefore,uCursorClipNow,t);
      if(any(greaterThanEqual(abs(pixel-rect.xy),rect.zw*0.5)))continue;
    }
    vec4 pose=mix(before,now,t);
    vec4 sampleColor=sampledCursorBody(pixel,pose,pose.z,pose.z);
    accumulated+=vec4(sampleColor.rgb*sampleColor.rgb*sampleColor.a,sampleColor.a)/float(count);
  }
  if(accumulated.a<=0.0)return vec4(0.0);
  return vec4(sqrt(max(accumulated.rgb/accumulated.a,vec3(0.0))),accumulated.a);
}

void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  // Convert UV to pixel coordinates
  // Match native integer top-left sample locations in the Y-up scene.
  vec2 pixel = floor(uv * vec2(uResolutionX, uResolutionY)) + vec2(0.0, 1.0);

  bool temporalActive=cursorTemporalActive();
  // Native bounds include the top/left pixel and exclude bottom/right.
  // Y is inverted here, so the lower Y bound is the exclusive edge.
  bool insideClip=uVideoClipEnabled<0.5 || (pixel.x>=uVideoClipMinX && pixel.x<uVideoClipMaxX
    && pixel.y>uVideoClipMinY && pixel.y<=uVideoClipMaxY);
  if(!insideClip && !temporalActive){outputColor=inputColor;return;}
  vec2 imagePosition=vec2(pixel.x,uResolutionY-pixel.y);
  float sweptRadius=cursorBodyRadius()*max(uCursorShutterStart.z,uCursorShutterNow.z)+1.5+3.0*uCursorFadeBlur;
  bool needsTemporal=temporalActive
    && all(greaterThanEqual(imagePosition,min(uCursorShutterStart.xy,uCursorShutterNow.xy)-vec2(sweptRadius)))
    && all(lessThanEqual(imagePosition,max(uCursorShutterStart.xy,uCursorShutterNow.xy)+vec2(sweptRadius)));

  vec2 cursorPixel = vec2(uCursorX, uCursorY) * vec2(uResolutionX, uResolutionY);

  float s = uCursorScale;

  // Early exit: bounding box check — skip all cursor math if pixel is far from cursor
  // Max radius covers ripple (48*s + some margin) and trail spread
  float maxRadius = max(60.0,cursorBodyRadius()) * s + 1.5 + 3.0*uCursorFadeBlur;

  // Also check distance to ripple and circle highlight centers
  vec2 ripplePixel = vec2(uRippleX, uRippleY) * vec2(uResolutionX, uResolutionY);
  vec2 circleHlPixel = vec2(uCircleHlX, uCircleHlY) * vec2(uResolutionX, uResolutionY);

  float distToCursor = length(pixel - cursorPixel);
  float distToRipple = length(pixel - ripplePixel);
  float distToCircleHl = length(pixel - circleHlPixel);

  // Check if any trail point is close
  float minTrailDist = 1e10;
  if (uTrailEnabled > 0.5) {
    int trailCount = int(uTrailCount);
    for (int i = 0; i < 30; i++) {
      if (i >= trailCount) break;
      vec2 tp = uTrailPoints[i].xy * vec2(uResolutionX, uResolutionY);
      minTrailDist = min(minTrailDist, length(pixel - tp));
    }
  }

  bool needsCursor = insideClip && (uCursorOpacity > 0.01) && (distToCursor < maxRadius);
  bool needsRipple = insideClip && (uRippleProgress > 0.001 && uRippleProgress < 1.0) && (distToRipple < maxRadius);
  bool needsCircleHl = insideClip && (uCircleHlProgress > 0.001 && uCircleHlProgress < 1.0) && (distToCircleHl < maxRadius);
  bool needsTrail = insideClip && (uTrailEnabled > 0.5) && (minTrailDist < 20.0 * s);

  if (!needsCursor && !needsTemporal && !needsRipple && !needsCircleHl && !needsTrail) {
    outputColor = inputColor;
    return;
  }

  vec4 result = inputColor;

  // Determine active cursor color (highlight when clicking)
  vec3 cursorColor = vec3(uCursorColorR, uCursorColorG, uCursorColorB);
  vec3 highlightColor = vec3(uHighlightColorR, uHighlightColorG, uHighlightColorB);
  vec3 rippleColor = vec3(uRippleColorR, uRippleColorG, uRippleColorB);

  // --- 1. Trail ---
  if (needsTrail) {
    int trailCount = int(uTrailCount);
    for (int i = 0; i < 30; i++) {
      if (i >= trailCount - 1) break; // skip last point (cursor itself)
      vec4 tp = uTrailPoints[i];
      vec2 trailPos = tp.xy * vec2(uResolutionX, uResolutionY);
      float progress = float(i) / float(trailCount);
      float trailRadius = (2.0 + progress * 4.0) * s;
      float trailAlpha = progress * uTrailOpacity * uCursorOpacity;
      float dist = length(pixel - trailPos);
      float mask = 1.0 - smoothstep(trailRadius - 0.5, trailRadius + 0.5, dist);
      if (mask * trailAlpha > 0.001) {
        vec4 trailColor = vec4(cursorColor, mask * trailAlpha);
        result = alphaBlend(result, trailColor);
      }
    }
  }

  // --- 2. Ripple effect ---
  if (needsRipple && uClickEffect > 1.5) { // ripple = 2
    float p = uRippleProgress;
    float dist = length(pixel - ripplePixel);

    // Outer ring: radius (8 + p*40) * s
    float outerRadius = (8.0 + p * 40.0) * s;
    float ring = abs(dist - outerRadius);
    float ringAlpha = 1.0 - smoothstep(0.0, 2.5, ring);
    float fade = (1.0 - p) * 0.6;
    if (ringAlpha * fade > 0.001) {
      vec4 rcolor = vec4(rippleColor, ringAlpha * fade);
      result = alphaBlend(result, rcolor);
    }

    // Inner fill (first 50%)
    if (p < 0.5) {
      float innerRadius = (4.0 + p * 20.0) * s;
      float innerAlpha = (0.5 - p) * 0.4;
      float innerMask = 1.0 - smoothstep(innerRadius - 0.5, innerRadius + 0.5, dist);
      if (innerMask * innerAlpha > 0.001) {
        vec4 innerColor = vec4(rippleColor, innerMask * innerAlpha);
        result = alphaBlend(result, innerColor);
      }
    }
  }

  // --- 3. Circle highlight ---
  if (needsCircleHl && uClickEffect > 0.5 && uClickEffect < 1.5) { // circle = 1
    float p = uCircleHlProgress;
    float dist = length(pixel - circleHlPixel);
    float radius = 20.0 * s;
    float alpha = (1.0 - p);

    // Fill
    float fillMask = 1.0 - smoothstep(radius - 0.5, radius + 0.5, dist);
    float fillAlpha = alpha * 0.24;
    if (fillMask * fillAlpha > 0.001) {
      vec4 hlFill = vec4(highlightColor, fillMask * fillAlpha);
      result = alphaBlend(result, hlFill);
    }

    // Stroke (2px)
    float ring = abs(dist - radius);
    float strokeMask = 1.0 - smoothstep(0.0, 2.0, ring);
    float strokeAlpha = alpha * 0.8;
    if (strokeMask * strokeAlpha > 0.001) {
      vec4 hlStroke = vec4(highlightColor, strokeMask * strokeAlpha);
      result = alphaBlend(result, hlStroke);
    }
  }

  vec2 imagePixel=vec2(pixel.x,uResolutionY-pixel.y);
  if(needsTemporal) {
    result=alphaBlend(result,temporalCursorBody(imagePixel));
  } else if(needsCursor && !temporalActive) {
    result=alphaBlend(result,sampledCursorBody(imagePixel,vec4(cursorPixel.x,uResolutionY-cursorPixel.y,s,uCursorRotation),uCursorTemporalEnabled>0.5?uCursorShutterStart.z:s,s));
  }

  outputColor = result;
}
`;
