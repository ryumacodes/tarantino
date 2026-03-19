// SDF-based cursor rendering fragment shader for postprocessing
// All cursor visuals computed procedurally — no texture needed.
// Matches Canvas 2D cursor overlay and export WGSL cursor rendering exactly.

export const cursorFragmentShader = `
uniform float uCursorX;          // screen UV (0-1)
uniform float uCursorY;          // screen UV (0-1)
uniform float uCursorScale;      // visual scale multiplier
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

// Distance from point p to line segment a-b
float sdSegment(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a;
  vec2 ba = b - a;
  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

// 7-vertex arrow polygon winding number test
// Returns 1.0 if inside, 0.0 if outside, with AA smoothstep
float arrowMask(vec2 p, float s) {
  // Arrow vertices (tip at origin)
  vec2 v0 = vec2(0.0, 0.0);
  vec2 v1 = vec2(0.0, 16.0 * s);
  vec2 v2 = vec2(4.0 * s, 12.0 * s);
  vec2 v3 = vec2(7.0 * s, 18.0 * s);
  vec2 v4 = vec2(9.5 * s, 17.0 * s);
  vec2 v5 = vec2(6.5 * s, 11.0 * s);
  vec2 v6 = vec2(12.0 * s, 11.0 * s);

  vec2 verts[7];
  verts[0] = v0; verts[1] = v1; verts[2] = v2;
  verts[3] = v3; verts[4] = v4; verts[5] = v5; verts[6] = v6;

  // Winding number test
  int wn = 0;
  for (int i = 0; i < 7; i++) {
    int j = (i + 1) < 7 ? i + 1 : 0;
    vec2 vi = verts[i];
    vec2 vj = verts[j];
    if (vi.y <= p.y) {
      if (vj.y > p.y) {
        float cross = (vj.x - vi.x) * (p.y - vi.y) - (p.x - vi.x) * (vj.y - vi.y);
        if (cross > 0.0) wn++;
      }
    } else {
      if (vj.y <= p.y) {
        float cross = (vj.x - vi.x) * (p.y - vi.y) - (p.x - vi.x) * (vj.y - vi.y);
        if (cross < 0.0) wn--;
      }
    }
  }

  // Compute min edge distance for AA
  float minDist = 1e10;
  for (int i = 0; i < 7; i++) {
    int j = (i + 1) < 7 ? i + 1 : 0;
    minDist = min(minDist, sdSegment(p, verts[i], verts[j]));
  }

  // Inside if winding number != 0, with 0.5px AA at boundary
  if (wn != 0) {
    return smoothstep(0.0, 0.5, minDist); // 1 inside, fade at edge
  } else {
    return 0.0;
  }
}

// Min distance to any of 7 edges (for stroke rendering)
float arrowEdgeDist(vec2 p, float s) {
  vec2 v0 = vec2(0.0, 0.0);
  vec2 v1 = vec2(0.0, 16.0 * s);
  vec2 v2 = vec2(4.0 * s, 12.0 * s);
  vec2 v3 = vec2(7.0 * s, 18.0 * s);
  vec2 v4 = vec2(9.5 * s, 17.0 * s);
  vec2 v5 = vec2(6.5 * s, 11.0 * s);
  vec2 v6 = vec2(12.0 * s, 11.0 * s);

  vec2 verts[7];
  verts[0] = v0; verts[1] = v1; verts[2] = v2;
  verts[3] = v3; verts[4] = v4; verts[5] = v5; verts[6] = v6;

  float minDist = 1e10;
  for (int i = 0; i < 7; i++) {
    int j = (i + 1) < 7 ? i + 1 : 0;
    minDist = min(minDist, sdSegment(p, verts[i], verts[j]));
  }
  return minDist;
}

// 2D rotation matrix
vec2 rotate2D(vec2 p, float angle) {
  float c = cos(angle);
  float s = sin(angle);
  return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

// Alpha blend: src over dst (premultiplied-aware)
vec4 alphaBlend(vec4 dst, vec4 src) {
  float outA = src.a + dst.a * (1.0 - src.a);
  if (outA < 0.001) return vec4(0.0);
  vec3 outRGB = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / outA;
  return vec4(outRGB, outA);
}

void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  // Convert UV to pixel coordinates
  vec2 pixel = uv * vec2(uResolutionX, uResolutionY);
  vec2 cursorPixel = vec2(uCursorX, uCursorY) * vec2(uResolutionX, uResolutionY);

  float s = uCursorScale;

  // Early exit: bounding box check — skip all cursor math if pixel is far from cursor
  // Max radius covers ripple (48*s + some margin) and trail spread
  float maxRadius = 60.0 * s;

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

  bool needsCursor = (uCursorOpacity > 0.01) && (distToCursor < maxRadius);
  bool needsRipple = (uRippleProgress > 0.001 && uRippleProgress < 1.0) && (distToRipple < maxRadius);
  bool needsCircleHl = (uCircleHlProgress > 0.001 && uCircleHlProgress < 1.0) && (distToCircleHl < maxRadius);
  bool needsTrail = (uTrailEnabled > 0.5) && (minTrailDist < 20.0 * s);

  if (!needsCursor && !needsRipple && !needsCircleHl && !needsTrail) {
    outputColor = inputColor;
    return;
  }

  vec4 result = inputColor;

  // Determine active cursor color (highlight when clicking)
  vec3 cursorColor = vec3(uCursorColorR, uCursorColorG, uCursorColorB);
  vec3 highlightColor = vec3(uHighlightColorR, uHighlightColorG, uHighlightColorB);
  vec3 rippleColor = vec3(uRippleColorR, uRippleColorG, uRippleColorB);
  vec3 activeCursorColor = uIsClicking > 0.5 ? highlightColor : cursorColor;

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

  // --- 4-6. Cursor body (shadow + shape + click ring) ---
  if (needsCursor) {
    // Local coordinates relative to cursor position
    vec2 local = pixel - cursorPixel;

    // Apply rotation
    float rotRad = uCursorRotation * 3.14159265 / 180.0;
    if (abs(rotRad) > 0.001) {
      local = rotate2D(local, -rotRad);
    }

    // Flip Y: postprocessing UV has Y-up (OpenGL), arrow vertices defined for Y-down (image space)
    local.y = -local.y;

    float opacity = uCursorOpacity;

    if (uCursorStyle < 0.5) {
      // --- Style 0: pointer ---
      // Shadow
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = alphaBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Body fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = alphaBlend(result, vec4(activeCursorColor, fill * opacity));
      }
      // Black stroke (1.5 * s width)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 1.5 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      // Only stroke outside the fill or at edges
      float strokeAlpha = strokeMask * (1.0 - fill * 0.5);
      if (strokeAlpha * opacity > 0.001) {
        result = alphaBlend(result, vec4(0.0, 0.0, 0.0, strokeAlpha * opacity));
      }

    } else if (uCursorStyle < 1.5) {
      // --- Style 1: circle ---
      // Shadow
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float dist = length(shadowLocal);
        float mask = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, dist);
        if (mask * shadowAlpha * opacity > 0.001) {
          result = alphaBlend(result, vec4(0.0, 0.0, 0.0, mask * shadowAlpha * opacity));
        }
      }
      // Circle body (gray 0.8 alpha)
      float dist = length(local);
      float mask = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, dist);
      if (mask * 0.8 * opacity > 0.001) {
        result = alphaBlend(result, vec4(0.5, 0.5, 0.5, mask * 0.8 * opacity));
      }

    } else if (uCursorStyle < 2.5) {
      // --- Style 2: filled (black fill, white stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = alphaBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Black fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = alphaBlend(result, vec4(0.0, 0.0, 0.0, fill * opacity));
      }
      // White stroke (2 * s)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 2.0 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      float strokeAlpha = strokeMask * (1.0 - fill * 0.5);
      if (strokeAlpha * opacity > 0.001) {
        result = alphaBlend(result, vec4(1.0, 1.0, 1.0, strokeAlpha * opacity));
      }

    } else if (uCursorStyle < 3.5) {
      // --- Style 3: outline (transparent fill, white stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowEdge = arrowEdgeDist(shadowLocal, s);
        float shadowStroke = 1.0 - smoothstep(0.0, 2.0 * s, shadowEdge);
        if (shadowStroke * shadowAlpha * opacity > 0.001) {
          result = alphaBlend(result, vec4(0.0, 0.0, 0.0, shadowStroke * shadowAlpha * opacity));
        }
      }
      // White stroke only (2 * s)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 2.0 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      if (strokeMask * opacity > 0.001) {
        result = alphaBlend(result, vec4(1.0, 1.0, 1.0, strokeMask * opacity));
      }

    } else {
      // --- Style 4: dotted (fill + dashed stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = alphaBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Body fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = alphaBlend(result, vec4(activeCursorColor, fill * opacity));
      }
      // Dashed black stroke (1.5 * s)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 1.5 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      // Dash pattern: use distance along perimeter approximation
      // Use pixel position modulo for dash effect
      float dashPeriod = 4.0 * s;
      float dashOn = step(0.5, fract((local.x + local.y) / dashPeriod));
      float strokeAlpha = strokeMask * dashOn * (1.0 - fill * 0.5);
      if (strokeAlpha * opacity > 0.001) {
        result = alphaBlend(result, vec4(0.0, 0.0, 0.0, strokeAlpha * opacity));
      }
    }

    // --- 6. Click highlight ring (when clickEffect=none & clicking) ---
    if (uIsClicking > 0.5 && uClickEffect < 0.5 && uCursorStyle > 0.5) {
      // Don't show for circle style — match Canvas 2D behavior
    }
    if (uIsClicking > 0.5 && uClickEffect < 0.5 && uCursorStyle < 0.5) {
      // For non-circle cursor styles with no click effect
      vec2 ringCenter = cursorPixel + vec2(5.0 * s, -5.0 * s);
      float dist = length(pixel - ringCenter);
      float ringRadius = 18.0 * s;
      float ring = abs(dist - ringRadius);
      float ringMask = 1.0 - smoothstep(0.0, 3.0 * s, ring);
      if (ringMask * 0.7 * opacity > 0.001) {
        result = alphaBlend(result, vec4(highlightColor, ringMask * 0.7 * opacity));
      }
    }
    // Also for filled/outline/dotted styles
    if (uIsClicking > 0.5 && uClickEffect < 0.5 && uCursorStyle > 1.5) {
      vec2 ringCenter = cursorPixel + vec2(5.0 * s, -5.0 * s);
      float dist = length(pixel - ringCenter);
      float ringRadius = 18.0 * s;
      float ring = abs(dist - ringRadius);
      float ringMask = 1.0 - smoothstep(0.0, 3.0 * s, ring);
      if (ringMask * 0.7 * opacity > 0.001) {
        result = alphaBlend(result, vec4(highlightColor, ringMask * 0.7 * opacity));
      }
    }
  }

  outputColor = result;
}
`;
