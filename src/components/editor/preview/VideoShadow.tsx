import React, { useMemo } from 'react';
import * as THREE from 'three';
import type { VisualSettings } from '../../../stores/editor';

interface VideoShadowProps {
  width: number;
  height: number;
  settings: VisualSettings;
}

export const VideoShadow: React.FC<VideoShadowProps> = ({ width, height, settings }) => {
  const shadowTexture = useMemo(() => {
    if (!settings.shadowEnabled) return null;

    const canvas = document.createElement('canvas');
    const size = 256;
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext('2d');
    if (!ctx) return null;

    // Create a soft shadow using radial gradient
    const blur = settings.shadowBlur / 100;
    const intensity = settings.shadowIntensity / 100;

    ctx.clearRect(0, 0, size, size);

    // Draw shadow box
    const padding = size * 0.15;
    const cornerRadius = (settings.cornerRadius / 50) * padding;

    ctx.shadowColor = `rgba(0, 0, 0, ${intensity})`;
    ctx.shadowBlur = size * blur * 0.5;
    ctx.shadowOffsetX = settings.shadowOffsetX * 2;
    ctx.shadowOffsetY = settings.shadowOffsetY * 2;

    ctx.fillStyle = 'rgba(0, 0, 0, 0)';
    ctx.beginPath();
    ctx.roundRect(padding, padding, size - padding * 2, size - padding * 2, cornerRadius);
    ctx.fill();

    // Draw actual shadow shape
    ctx.shadowColor = 'transparent';
    ctx.fillStyle = `rgba(0, 0, 0, ${intensity})`;
    ctx.filter = `blur(${size * blur * 0.15}px)`;
    ctx.beginPath();
    ctx.roundRect(padding, padding, size - padding * 2, size - padding * 2, cornerRadius);
    ctx.fill();

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;
    return texture;
  }, [settings.shadowEnabled, settings.shadowIntensity, settings.shadowBlur, settings.shadowOffsetX, settings.shadowOffsetY, settings.cornerRadius]);

  if (!settings.shadowEnabled || !shadowTexture) return null;

  const shadowScale = 1.2;
  return (
    <mesh position={[settings.shadowOffsetX * 0.01, -settings.shadowOffsetY * 0.01, -0.1]}>
      <planeGeometry args={[width * shadowScale, height * shadowScale]} />
      <meshBasicMaterial
        map={shadowTexture}
        transparent
        toneMapped={false}
        depthWrite={false}
      />
    </mesh>
  );
};
