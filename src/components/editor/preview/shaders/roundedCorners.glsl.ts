// Rounded corners shader for video frame
export const roundedCornersVertexShader = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const roundedCornersFragmentShader = `
uniform sampler2D map;
uniform float cornerRadius;
uniform float aspectRatio;
varying vec2 vUv;

float roundedBoxSDF(vec2 center, vec2 size, float radius) {
  vec2 q = abs(center) - size + radius;
  return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

void main() {
  vec2 size = vec2(0.5, 0.5);
  vec2 center = vUv - 0.5;

  // Adjust for aspect ratio
  center.x *= aspectRatio;
  size.x *= aspectRatio;

  float radius = cornerRadius * min(size.x, size.y) * 2.0;
  float d = roundedBoxSDF(center, size, radius);

  if (d > 0.0) {
    discard;
  }

  vec4 texColor = texture2D(map, vUv);
  gl_FragColor = texColor;
}
`;
