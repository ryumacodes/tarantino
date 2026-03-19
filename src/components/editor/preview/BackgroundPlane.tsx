import React, { useRef, useMemo } from 'react';
import * as THREE from 'three';
import type { VisualSettings } from '../../../stores/editor';

interface BackgroundPlaneProps {
  width: number;
  height: number;
  settings: VisualSettings;
}

export const BackgroundPlane: React.FC<BackgroundPlaneProps> = ({ width, height, settings }) => {
  const meshRef = useRef<THREE.Mesh>(null);

  // Create gradient texture
  const gradientTexture = useMemo(() => {
    if (settings.backgroundType !== 'gradient') return null;

    const canvas = document.createElement('canvas');
    canvas.width = 512;
    canvas.height = 512;
    const ctx = canvas.getContext('2d');
    if (!ctx) return null;

    let gradient: CanvasGradient;
    if (settings.gradientDirection === 'radial') {
      gradient = ctx.createRadialGradient(256, 256, 0, 256, 256, 360);
    } else if (settings.gradientDirection === 'to-right') {
      gradient = ctx.createLinearGradient(0, 0, 512, 0);
    } else if (settings.gradientDirection === 'to-bottom') {
      gradient = ctx.createLinearGradient(0, 0, 0, 512);
    } else {
      // to-bottom-right
      gradient = ctx.createLinearGradient(0, 0, 512, 512);
    }

    settings.gradientStops.forEach(stop => {
      gradient.addColorStop(stop.position / 100, stop.color);
    });

    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, 512, 512);

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;
    return texture;
  }, [settings.backgroundType, settings.gradientDirection, settings.gradientStops]);

  return (
    <mesh ref={meshRef} position={[0, 0, -1]}>
      <planeGeometry args={[width * 1.5, height * 1.5]} />
      {settings.backgroundType === 'gradient' && gradientTexture ? (
        <meshBasicMaterial map={gradientTexture} toneMapped={false} />
      ) : (
        <meshBasicMaterial color={settings.backgroundColor} toneMapped={false} />
      )}
    </mesh>
  );
};
