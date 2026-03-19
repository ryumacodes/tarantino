// Motion blur fragment shader - Screen Studio style
// Separate channels for pan blur and zoom blur
export const motionBlurFragmentShader = `
uniform float uPanIntensity;
uniform float uZoomIntensity;
uniform float uVelocityX;
uniform float uVelocityY;
uniform float uVelocityScale;

void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  // Pan blur: directional blur from lateral camera movement
  float panSpeed = sqrt(uVelocityX * uVelocityX + uVelocityY * uVelocityY);
  float panBlur = panSpeed * uPanIntensity * 0.008;
  panBlur = min(panBlur, 0.012);

  // Zoom blur: radial blur from scale changes (subtle)
  float zoomSpeed = abs(uVelocityScale);
  float zoomBlur = zoomSpeed * uZoomIntensity * 0.006;
  zoomBlur = min(zoomBlur, 0.008);

  // Early exit if no significant blur
  if (panBlur < 0.001 && zoomBlur < 0.001) {
    outputColor = inputColor;
    return;
  }

  // Pan blur direction (normalized velocity)
  vec2 panDir = vec2(uVelocityX, uVelocityY);
  float dirMag = length(panDir);
  if (dirMag > 0.001) {
    panDir = panDir / dirMag;
  } else {
    panDir = vec2(0.0, 0.0);
  }

  // Multi-sample blur
  vec4 color = vec4(0.0);
  const int samples = 12;
  float totalWeight = 0.0;

  for (int i = 0; i < samples; i++) {
    float t = float(i) / float(samples - 1) - 0.5;
    float weight = 1.0 - abs(t) * 0.5;

    // Directional pan blur offset
    vec2 offset = panDir * panBlur * t;

    // Radial zoom blur offset (emanates from center)
    vec2 fromCenter = uv - vec2(0.5);
    offset += fromCenter * zoomBlur * t;

    vec2 sampleUv = clamp(uv + offset, vec2(0.0), vec2(1.0));
    color += texture2D(inputBuffer, sampleUv) * weight;
    totalWeight += weight;
  }

  outputColor = color / totalWeight;
}
`;
