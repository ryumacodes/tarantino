import type {CameraPreviewSource} from './CameraPreviewSource';
import React, { useEffect, useMemo } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { BlendFunction, Effect, EffectAttribute } from 'postprocessing';
import { DataTexture, FloatType, RedFormat, Uniform, Vector2, type Texture, type WebGLRenderer, type WebGLRenderTarget } from 'three';
import { LoupePreviewLayers } from './LoupePreviewLayers';
import type { CursorEffectImpl } from './CursorEffect';
import type { Scene, Camera } from 'three';
import { LoupePyramid } from './LoupePyramid';
import { loupeFiltering } from './loupeFiltering';
import { loupePoolLighting } from './loupePoolLighting';
import { loupeLighting } from './loupeLighting';
import { loupeNoise } from './loupeNoise';
import { useEditorStore } from '../../../stores/editor';
import { previewSample } from '../../../utils/timeline/previewTimeline';
import { cameraFrameIndex } from '../../../utils/motion/zoomTrajectory';
import { projectLoupeToPreview, type VideoTransform } from '../../../utils/motion/loupe';
import { useLoupeTrajectory } from './hooks/useCameraTrajectory';

const shader = `
uniform vec2 lensCenter;
uniform float lensRadius;
uniform float lensZoom;
uniform float lensPresence;
uniform float lensBevel;
uniform float lensAberration;
uniform float lensSourcePixelScale;
uniform float lensShadowAlpha;
uniform float lensShadowReach;
uniform vec2 lensLightDirection;
uniform float lensLightTilt;
${loupeFiltering}
float orderedThreshold(vec2 pixel) {
  vec2 bits = mod(floor(pixel),8.0);
  float value = 0.0;
  float weight = 16.0;
  for (int bit=0;bit<3;bit++) {
    vec2 low = mod(bits,2.0);
    value += (mod(low.x+low.y,2.0)*2.0+low.y)*weight;
    bits = floor(bits/2.0);
    weight *= 0.25;
  }
  return value/64.0;
}
float spectrumIntegral(float value) {
  float x=clamp(value,-1.0,1.0);
  float squared=x*x;
  return x*(1.0-squared*(2.0/3.0)+squared*squared*0.2);
}
float bandIntegral(float start,float end,float center,float width) {
  return width*(spectrumIntegral((end-center)/width)-spectrumIntegral((start-center)/width));
}
vec4 sceneRead(vec2 uv,vec2 backdropUv,float radius,vec2 pixel) {
  vec4 front=filteredRead(uv,radius,pixel);
  if(lensSeparateLayers<=0.0 || front.a>=1.0) return front;
  return front+texture2D(lensBackground,clamp(backdropUv,vec2(0.0),vec2(1.0)))*(1.0-front.a);
}
vec4 spectralSample(vec2 sampleUv,vec2 backdropUv,vec2 splitUv,float splitPixels,vec2 pixel,float blurRadius) {
  if (splitPixels<0.75) return sceneRead(sampleUv,backdropUv,blurRadius,pixel);
  int reads=int(clamp(ceil(splitPixels),3.0,16.0));
  float interval=2.0/float(reads);
  vec3 color=vec3(0.0),weightSum=vec3(0.0);
  float alpha=0.0;
  for(int index=0;index<16;index++) {
    if(index>=reads) break;
    float end=1.0-float(index)*interval;
    float start=end-interval;
    float phase=fract(orderedThreshold(pixel)+float(index)*0.61803398875);
    vec2 samplePosition=sampleUv+splitUv*(end-phase*interval);
    vec4 sampled=sceneRead(samplePosition,backdropUv+splitUv*lensZoom*(end-phase*interval),blurRadius,pixel);
    vec3 rgb=clamp(sampled.rgb,vec3(0.0),vec3(sampled.a));
    vec3 linear=rgb*rgb/max(sampled.a,0.00001);
    vec3 weights=vec3(bandIntegral(start,end,0.85,0.7)+0.3*bandIntegral(start,end,-0.95,0.35),
      bandIntegral(start,end,0.05,0.65),bandIntegral(start,end,-0.75,0.65));
    color+=linear*weights;
    weightSum+=weights;
    alpha+=sampled.a;
  }
  alpha/=float(reads);
  return vec4(sqrt(max(color/weightSum*alpha,vec3(0.0))),alpha);
}
float outerShadow(vec2 relative,float radius) {
  if (lensShadowAlpha <= 0.001) return 0.0;
  float reach = max(lensShadowReach*resolution.x,lensSourcePixelScale*resolution.x);
  float hoverHeight = min(reach,radius*0.25);
  float throwDistance = min(lensLightTilt*hoverHeight,reach*0.45);
  float distance = length(relative+lensLightDirection*throwDistance);
  float effectiveReach = max(reach-throwDistance,reach*0.4);
  float falloff = clamp(1.0-(distance-radius)/effectiveReach,0.0,1.0);
  float facing = dot(relative,lensLightDirection)/max(length(relative),0.001*lensSourcePixelScale*resolution.x);
  return lensShadowAlpha*mix(1.0,0.6,(facing+1.0)*0.5)*falloff*falloff;
}
vec3 rimReflection(vec2 position,float radius,float bevel) {
  float strength=min(bevel/(lensSourcePixelScale*resolution.x)*0.5,1.0)*lensPresence;
  float distance=length(position);
  float band=max(min(bevel*1.5,radius),lensSourcePixelScale*resolution.x);
  if(strength<=0.001 || distance>=radius || distance<0.001*lensSourcePixelScale*resolution.x || distance<=radius-band) return vec3(0.0);
  float t=clamp((distance-radius+band)/band,0.0,1.0);
  float squared=t*t;
  float slope=min(squared*inversesqrt(1.0-min(squared,0.99)),6.0);
  float cosine=inversesqrt(1.0+slope*slope);
  float weight=clamp((0.04+0.96*pow(1.0-cosine,5.0))*smoothstep(1.0,1.5,slope)*strength,0.0,1.0);
  if(weight<=0.004) return vec3(0.0);
  float outward=min(radius*0.5*slope/max(slope*slope-1.0,0.05),radius);
  return vec3(position/distance*max(distance+outward,radius+lensSourcePixelScale*resolution.x),weight);
}
${loupeLighting}
${loupePoolLighting}
void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  vec2 position=floor(uv*resolution)+0.5;
  vec2 relative=position-lensCenter*resolution;
  float radiusPx = lensRadius * resolution.x;
  float distancePx = length(relative);
  float coverage = (1.0-smoothstep(radiusPx-0.5,radiusPx+0.5,distancePx))*lensPresence;
  if (lensPresence <= 0.0 || radiusPx <= 0.0) {
    outputColor = inputColor;
    return;
  }
  if (coverage <= 0.0) {
    if(lensLighting>0.0 && lensBevel>0.0) {outputColor=shadePool(inputColor,relative,0.0);return;}
    float shadow = outerShadow(relative,radiusPx);
    vec2 pixel = floor(vec2(uv.x,1.0-uv.y)*resolution);
    float noise = orderedThreshold(pixel+vec2(4.0))/255.0*min(shadow*64.0,1.0);
    float opacity = clamp(shadow+(orderedThreshold(pixel)-0.5)*(1.5/255.0)*shadow,0.0,1.0);
    outputColor = vec4(inputColor.rgb*(1.0-opacity)+vec3(noise),inputColor.a);
    return;
  }
  vec2 sampled = refractedPosition(position);
  vec2 sampleUv = sampled/resolution;
  vec2 ordinary = lensCenter*resolution+relative/lensZoom;
  vec2 splitPx=(ordinary-sampled)*lensZoom*0.35*lensAberration;
  float bevelPx = lensBevel*resolution.x;
  vec2 pixel=floor(vec2(uv.x,1.0-uv.y)*resolution);
  vec4 magnified=spectralSample(sampleUv,fullRefractedPosition(position)/resolution,splitPx/resolution/lensZoom,length(splitPx),pixel,rimBlurRadius(position,sampled));
  vec3 reflection=rimReflection(relative,radiusPx,bevelPx);
  if(reflection.z>0.0) {
    vec2 plainUv=clamp(lensCenter+reflection.xy/resolution,vec2(0.0),vec2(1.0));
    vec4 plain=texture2D(inputBuffer,plainUv);
    vec4 reflected=vec4(plain.rgb*(1.0-outerShadow(reflection.xy,radiusPx)),plain.a);
    magnified=mix(magnified,reflected,reflection.z);
  }
  vec4 composed=mix(inputColor,magnified,coverage);
  if(lensLighting>0.0 && lensBevel>0.0) composed=shadePool(composed,relative,coverage);
  outputColor=shadeRim(composed,relative,coverage,pixel);
}`;

export class LoupeEffectImpl extends Effect {
  private pyramid:LoupePyramid | null = null;
  private previewLayers:LoupePreviewLayers | null = null;
  private sceneSource:{scene:Scene;camera:Camera;cursor:React.MutableRefObject<CursorEffectImpl | null>; cameraSource?:CameraPreviewSource} | null = null;
  setSceneSource(source:typeof this.sceneSource) { this.sceneSource=source; }
  private readonly noise:DataTexture;
  private sceneLayers:{foreground:Texture;background:Texture} | null = null;
  /** Supplied layers must contain premultiplied RGBA at the input resolution. */
  setSceneLayers(layers:{foreground:Texture;background:Texture} | null) { this.sceneLayers=layers; }
  constructor() {
    const noise=new DataTexture(loupeNoise,32,32,RedFormat,FloatType);
    noise.needsUpdate=true;
    super('MagnifierContent',shader,{
      blendFunction:BlendFunction.NORMAL,
      // A separate pass is required: sample the completed cursor/effects image,
      // not the original scene texture used by a merged EffectPass.
      attributes:EffectAttribute.CONVOLUTION,
      uniforms:new Map<string,Uniform>([
        ['lensLighting',new Uniform(0)],['lensShineExponent',new Uniform(8)],
        ['lensForeground',new Uniform(null)],['lensBackground',new Uniform(null)],['lensSeparateLayers',new Uniform(0)],
        ['lensPyramid',new Uniform(null)],['lensNoise',new Uniform(noise)],['lensMipCount',new Uniform(1)],
        ['lensCenter',new Uniform(new Vector2(0.5,0.5))],
        ['lensShadowAlpha',new Uniform(0)],['lensShadowReach',new Uniform(0)],
        ['lensLightDirection',new Uniform(new Vector2(0,1))],['lensLightTilt',new Uniform(0)],
        ['lensSourcePixelScale',new Uniform(1)],['lensAberration',new Uniform(0)],['lensBevel',new Uniform(0)],['lensRadius',new Uniform(0)],['lensZoom',new Uniform(1)],['lensPresence',new Uniform(0)],
      ]),
    });
    this.noise=noise;
  }
  override update(renderer:WebGLRenderer,input:WebGLRenderTarget) {
    if (this.sceneSource) {
      this.sceneLayers=null;
      if (this.uniforms.get('lensPresence')!.value>0 && (this.uniforms.get('lensBevel')!.value>0 || this.sceneSource.cameraSource)) {
        this.previewLayers ??= new LoupePreviewLayers(input.width,input.height);
        const {scene,camera,cursor,cameraSource}=this.sceneSource;
        this.sceneLayers=this.previewLayers.capture(renderer,scene,camera,input.width,input.height,
          cursor.current?.uniforms ?? null,cameraSource);
      }
    }
    this.uniforms.get('lensForeground')!.value=this.sceneLayers?.foreground ?? input.texture;
    this.uniforms.get('lensBackground')!.value=this.sceneLayers?.background ?? input.texture;
    this.uniforms.get('lensSeparateLayers')!.value=this.sceneLayers ? 1 : 0;
    if(this.uniforms.get('lensPresence')!.value<=0 || this.uniforms.get('lensBevel')!.value<=0) return;
    if(!this.pyramid || this.pyramid.width!==input.width || this.pyramid.height!==input.height) {
      this.pyramid?.dispose();
      this.pyramid=new LoupePyramid(input.width,input.height);
    }
    this.pyramid.update(renderer,this.sceneLayers?.foreground ?? input.texture);
    this.uniforms.get('lensPyramid')!.value=this.pyramid.target.texture;
    this.uniforms.get('lensMipCount')!.value=this.pyramid.levels.length;
  }
  override dispose() {
    this.previewLayers?.dispose();this.previewLayers=null;
    this.pyramid?.dispose();this.pyramid=null;
    this.noise.dispose();super.dispose();
  }
}

export function LoupeEffect({videoTransformRef,cursorEffectRef,cameraSource}: {
  cameraSource?:CameraPreviewSource;
  videoTransformRef:React.MutableRefObject<VideoTransform>;
  cursorEffectRef:React.MutableRefObject<CursorEffectImpl | null>;
}) {
  const effect = useMemo(()=>new LoupeEffectImpl(),[]);
  const {scene,camera}=useThree();
  useEffect(()=>{
    effect.setSceneSource({scene,camera,cursor:cursorEffectRef,cameraSource});
    return ()=>effect.setSceneSource(null);
  },[effect,scene,camera,cursorEffectRef,cameraSource]);
  const unsupported = useEditorStore(state => !cameraSource && state.hasWebcam && state.visualSettings.webcamEnabled !== false
    ? 'Magnifier camera-layer preview is not available yet'
    : null);
  const {frames,error} = useLoupeTrajectory(!unsupported);
  useEffect(()=>()=>effect.dispose(),[effect]);
  useFrame(()=>{
    const presence = effect.uniforms.get('lensPresence')!;
    presence.value = 0;
    const state = useEditorStore.getState();
    const sample = previewSample(state,state.currentTime);
    if (unsupported || error || sample.sourceTime === null || !frames.length) return;
    const frame = frames[cameraFrameIndex(sample.sourceTime,frames.length)];
    if (!frame || frame.presence <= 0 || frame.radius <= 0) return;
    const projected = projectLoupeToPreview(frame,{
      width:state.videoWidth || state.displayResolution?.width || 1920,
      height:state.videoHeight || state.displayResolution?.height || 1080,
    },state.visualSettings.sourceCrop,videoTransformRef.current);
    effect.uniforms.get('lensCenter')!.value.set(projected.x,projected.y);
    effect.uniforms.get('lensRadius')!.value = projected.radius;
    effect.uniforms.get('lensShadowAlpha')!.value = Math.max(0,Math.min(1,frame.shadow_alpha));
    effect.uniforms.get('lensShadowReach')!.value = projected.shadowReach;
    effect.uniforms.get('lensLightDirection')!.value.set(...projected.lightDirection);
    effect.uniforms.get('lensLightTilt')!.value = projected.lightTilt;
    effect.uniforms.get('lensLighting')!.value = 1;
    effect.uniforms.get('lensShineExponent')!.value = projected.shineExponent;
    effect.uniforms.get('lensSourcePixelScale')!.value = projected.sourcePixelScale;
    effect.uniforms.get('lensAberration')!.value = Math.max(0,Math.min(1,frame.chromatic_aberration));
    effect.uniforms.get('lensBevel')!.value = projected.radius*Math.max(0,Math.min(1,frame.bevel));
    effect.uniforms.get('lensZoom')!.value = frame.zoom;
    presence.value = frame.presence;
  });
  return <>
    <primitive object={effect} dispose={null} />
    {(unsupported || error) && <Html center><div role="alert">{unsupported ?? error}</div></Html>}
  </>;
}
