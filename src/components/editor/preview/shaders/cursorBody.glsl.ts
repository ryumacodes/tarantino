export const cursorBodyShader = `
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

// Cursor colors are straight alpha; isolated scene layers are premultiplied.
vec4 alphaBlend(vec4 dst, vec4 src) {
  float outA = src.a + dst.a * (1.0 - src.a);
  if (outA < 0.001) return vec4(0.0);
  #ifdef CURSOR_PREMULTIPLIED_INPUT
  return vec4(src.rgb * src.a + dst.rgb * (1.0 - src.a), outA);
  #endif
  vec3 outRGB = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / outA;
  return vec4(outRGB, outA);
}


vec4 cursorBodyBlend(vec4 dst,vec4 src) {
  float alpha=src.a+dst.a*(1.0-src.a);
  if(alpha<0.001)return vec4(0.0);
  return vec4((src.rgb*src.a+dst.rgb*dst.a*(1.0-src.a))/alpha,alpha);
}
vec4 cursorBody(vec2 pixel,vec4 pose,vec3 activeCursorColor) {
  vec4 result=vec4(0.0);float s=pose.z;
  if(length(pixel-pose.xy)>=60.0*s || uCursorOpacity<=0.01)return result;
    // Local coordinates relative to cursor position
    vec2 local = pixel - pose.xy;


    // Apply rotation
    float rotRad = pose.w * 3.14159265 / 180.0;
    if (abs(rotRad) > 0.001) {
      local = rotate2D(local, -rotRad);
    }


    float opacity = uCursorOpacity;

    if (uCursorStyle < 0.5) {
      // --- Style 0: pointer ---
      // Shadow
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Body fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = cursorBodyBlend(result, vec4(activeCursorColor, fill * opacity));
      }
      // Black stroke (1.5 * s width)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 1.5 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      // Only stroke outside the fill or at edges
      float strokeAlpha = strokeMask * (1.0 - fill * 0.5);
      if (strokeAlpha * opacity > 0.001) {
        result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, strokeAlpha * opacity));
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
          result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, mask * shadowAlpha * opacity));
        }
      }
      // Circle body (gray 0.8 alpha)
      float dist = length(local);
      float mask = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, dist);
      if (mask * 0.8 * opacity > 0.001) {
        result = cursorBodyBlend(result, vec4(0.5, 0.5, 0.5, mask * 0.8 * opacity));
      }

    } else if (uCursorStyle < 2.5) {
      // --- Style 2: filled (black fill, white stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Black fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, fill * opacity));
      }
      // White stroke (2 * s)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 2.0 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      float strokeAlpha = strokeMask * (1.0 - fill * 0.5);
      if (strokeAlpha * opacity > 0.001) {
        result = cursorBodyBlend(result, vec4(1.0, 1.0, 1.0, strokeAlpha * opacity));
      }

    } else if (uCursorStyle < 3.5) {
      // --- Style 3: outline (transparent fill, white stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowEdge = arrowEdgeDist(shadowLocal, s);
        float shadowStroke = 1.0 - smoothstep(0.0, 2.0 * s, shadowEdge);
        if (shadowStroke * shadowAlpha * opacity > 0.001) {
          result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, shadowStroke * shadowAlpha * opacity));
        }
      }
      // White stroke only (2 * s)
      float edgeDist = arrowEdgeDist(local, s);
      float strokeWidth = 2.0 * s;
      float strokeMask = 1.0 - smoothstep(0.0, strokeWidth, edgeDist);
      if (strokeMask * opacity > 0.001) {
        result = cursorBodyBlend(result, vec4(1.0, 1.0, 1.0, strokeMask * opacity));
      }

    } else {
      // --- Style 4: dotted (fill + dashed stroke) ---
      float shadowAlpha = (uShadowIntensity / 100.0) * 0.5;
      if (shadowAlpha > 0.001) {
        vec2 shadowLocal = local - vec2(2.0 * s, 2.0 * s);
        float shadowFill = arrowMask(shadowLocal, s);
        if (shadowFill * shadowAlpha * opacity > 0.001) {
          result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, shadowFill * shadowAlpha * opacity));
        }
      }
      // Body fill
      float fill = arrowMask(local, s);
      if (fill > 0.001) {
        result = cursorBodyBlend(result, vec4(activeCursorColor, fill * opacity));
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
        result = cursorBodyBlend(result, vec4(0.0, 0.0, 0.0, strokeAlpha * opacity));
      }
    }

  return result;
}
`;
