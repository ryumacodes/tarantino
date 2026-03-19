import React, { useMemo, forwardRef } from 'react';
import { useFrame } from '@react-three/fiber';
import { Effect, BlendFunction } from 'postprocessing';
import * as THREE from 'three';
import { motionBlurFragmentShader } from './shaders/motionBlur.glsl';

class MotionBlurEffectImpl extends Effect {
  constructor() {
    super('MotionBlurEffect', motionBlurFragmentShader, {
      blendFunction: BlendFunction.NORMAL,
      uniforms: new Map<string, THREE.Uniform>([
        ['uPanIntensity', new THREE.Uniform(0.0)],
        ['uZoomIntensity', new THREE.Uniform(0.0)],
        ['uVelocityX', new THREE.Uniform(0.0)],
        ['uVelocityY', new THREE.Uniform(0.0)],
        ['uVelocityScale', new THREE.Uniform(0.0)],
      ]),
    });
  }

  update(
    _renderer: THREE.WebGLRenderer,
    _inputBuffer: THREE.WebGLRenderTarget,
    _deltaTime: number
  ) {
    // Uniforms are updated externally via setUniforms
  }

  setUniforms(panIntensity: number, zoomIntensity: number, velocityX: number, velocityY: number, velocityScale: number) {
    this.uniforms.get('uPanIntensity')!.value = panIntensity;
    this.uniforms.get('uZoomIntensity')!.value = zoomIntensity;
    this.uniforms.get('uVelocityX')!.value = velocityX;
    this.uniforms.get('uVelocityY')!.value = velocityY;
    this.uniforms.get('uVelocityScale')!.value = velocityScale;
  }
}

interface MotionBlurProps {
  panIntensity: number; // 0-100
  zoomIntensity: number; // 0-100
  velocityRef: React.MutableRefObject<{ scale: number; x: number; y: number }>;
  enabled: boolean;
}

export const MotionBlurEffect = forwardRef<MotionBlurEffectImpl, MotionBlurProps>(
  ({ panIntensity, zoomIntensity, velocityRef, enabled }, ref) => {
    const effect = useMemo(() => new MotionBlurEffectImpl(), []);

    useFrame(() => {
      if (enabled && effect) {
        effect.setUniforms(
          panIntensity / 100,
          zoomIntensity / 100,
          velocityRef.current.x,
          velocityRef.current.y,
          velocityRef.current.scale
        );
      } else if (effect) {
        effect.setUniforms(0, 0, 0, 0, 0);
      }
    });

    React.useImperativeHandle(ref, () => effect, [effect]);

    return <primitive object={effect} />;
  }
);

MotionBlurEffect.displayName = 'MotionBlurEffect';
