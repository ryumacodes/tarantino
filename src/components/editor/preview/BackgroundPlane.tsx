import React, { useEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { WALLPAPERS, type VisualSettings } from '../../../stores/editor';

interface BackgroundPlaneProps {
  width: number;
  height: number;
  settings: VisualSettings;
}

export const BackgroundPlane: React.FC<BackgroundPlaneProps> = ({ width, height, settings }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const [imageTexture, setImageTexture] = useState<THREE.Texture | null>(null);

  const wallpaper = settings.backgroundType === 'wallpaper' && settings.wallpaperId
    ? WALLPAPERS[settings.wallpaperId as keyof typeof WALLPAPERS]
    : null;

  // Create gradient/wallpaper texture
  const backgroundTexture = useMemo(() => {
    const wallpaper = settings.backgroundType === 'wallpaper' && settings.wallpaperId
      ? WALLPAPERS[settings.wallpaperId as keyof typeof WALLPAPERS]
      : null;
    const isWallpaperGradient = wallpaper?.type === 'gradient';
    if (settings.backgroundType !== 'gradient' && !isWallpaperGradient) return null;
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

    const stops = isWallpaperGradient
      ? [...(wallpaper as { type: 'gradient'; colors: readonly string[] }).colors].map((color, i, colors) => ({
        color,
        position: (i / (colors.length - 1)) * 100
      }))
      : settings.gradientStops;

    stops.forEach(stop => {
      gradient.addColorStop(stop.position / 100, stop.color);
    });

    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, 512, 512);

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;
    return texture;
  }, [settings.backgroundType, settings.gradientDirection, settings.gradientStops, settings.wallpaperId]);

  const solidColor = wallpaper?.type === 'solid'
    ? (wallpaper as { type: 'solid'; color: string }).color
    : settings.backgroundColor;

  useEffect(() => {
    const imageSrc = settings.customBackgroundImage;
    (window as any).__TARANTINO_BACKGROUND_STATE = {
      backgroundType: settings.backgroundType,
      wallpaperId: settings.wallpaperId,
      hasCustomBackgroundImage: Boolean(imageSrc),
      customBackgroundImageLength: imageSrc?.length ?? 0,
      planeWidth: width,
      planeHeight: height,
      renderer: imageSrc && settings.backgroundType === 'wallpaper' && !settings.wallpaperId
        ? 'custom-wallpaper-image'
        : settings.wallpaperId
          ? 'preset-wallpaper'
          : settings.backgroundType,
    };
    if (settings.backgroundType !== 'wallpaper' || settings.wallpaperId || !imageSrc) {
      console.info('[Wallpaper Image] preview using non-custom background', {
        backgroundType: settings.backgroundType,
        wallpaperId: settings.wallpaperId,
        hasCustomImage: Boolean(imageSrc),
      });
      setImageTexture(null);
      return;
    }

    console.info('[Wallpaper Image] preview loading custom image', {
      dataUrlLength: imageSrc.length,
      prefix: imageSrc.slice(0, 48),
      planeWidth: width,
      planeHeight: height,
    });
    let cancelled = false;
    const image = new window.Image();
    image.crossOrigin = 'anonymous';
    image.onload = () => {
      if (cancelled) return;
      console.info('[Wallpaper Image] preview image decoded', {
        naturalWidth: image.width,
        naturalHeight: image.height,
      });
      const aspect = Math.max(width / Math.max(height, 0.001), 0.001);
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.round(1024 * aspect));
      canvas.height = 1024;
      const ctx = canvas.getContext('2d');
      if (!ctx) {
        console.warn('[Wallpaper Image] preview canvas context unavailable');
        return;
      }
      const scale = Math.max(canvas.width / image.width, canvas.height / image.height);
      const drawW = image.width * scale;
      const drawH = image.height * scale;
      console.info('[Wallpaper Image] preview texture prepared', {
        canvasWidth: canvas.width,
        canvasHeight: canvas.height,
        drawWidth: Math.round(drawW),
        drawHeight: Math.round(drawH),
      });
      ctx.drawImage(image, (canvas.width - drawW) / 2, (canvas.height - drawH) / 2, drawW, drawH);
      const texture = new THREE.CanvasTexture(canvas);
      texture.needsUpdate = true;
      setImageTexture(texture);
    };
    image.onerror = () => {
      if (cancelled) return;
      console.warn('[Wallpaper Image] preview failed to load custom image', {
        dataUrlLength: imageSrc.length,
        prefix: imageSrc.slice(0, 48),
      });
      setImageTexture(null);
    };
    image.src = imageSrc;

    return () => {
      cancelled = true;
      setImageTexture((texture) => {
        texture?.dispose();
        return null;
      });
    };
  }, [settings.backgroundType, settings.customBackgroundImage, settings.wallpaperId, width, height]);

  return (
    <mesh ref={meshRef} position={[0, 0, -1]}>
      <planeGeometry args={[width * 1.5, height * 1.5]} />
      {imageTexture ? (
        <meshBasicMaterial key="custom-image" map={imageTexture} color="#ffffff" toneMapped={false} />
      ) : backgroundTexture ? (
        <meshBasicMaterial key="generated-texture" map={backgroundTexture} color="#ffffff" toneMapped={false} />
      ) : (
        <meshBasicMaterial key="solid-color" color={solidColor} toneMapped={false} />
      )}
    </mesh>
  );
};
