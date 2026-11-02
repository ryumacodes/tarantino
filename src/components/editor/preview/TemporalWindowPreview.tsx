import {useEffect,useMemo,type MutableRefObject,type RefObject} from 'react';
import {useFrame} from '@react-three/fiber';
import {Group,Mesh,PlaneGeometry,ShaderMaterial} from 'three';
import {TemporalWindowPass,type TemporalWindowInputs} from './TemporalWindowPass';

export interface TemporalWindowFrame {
  width:number;height:number;input:TemporalWindowInputs;
}

/** Replace the original window group with one temporally composed scene layer. */
export function TemporalWindowPreview({original,activeRef,canvasWidth,canvasHeight,getFrame}:{
  original:RefObject<Group|null>;activeRef:MutableRefObject<boolean>;
  canvasWidth:number;canvasHeight:number;getFrame:()=>TemporalWindowFrame|null;
}) {
  const resources=useMemo(()=>{
    const pass=new TemporalWindowPass(1,1);
    const material=new ShaderMaterial({transparent:true,depthWrite:false,toneMapped:false,
      uniforms:{layer:{value:pass.target.texture}},
      vertexShader:'varying vec2 uvOut; void main(){uvOut=uv;gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0);}',
      fragmentShader:`uniform sampler2D layer; varying vec2 uvOut;
      void main(){
        vec4 value=texture2D(layer,uvOut);
        vec3 rgb=value.a>0.0?value.rgb/value.a:vec3(0.0);
        rgb=mix(rgb/12.92,pow((rgb+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),rgb));
        gl_FragColor=vec4(rgb,value.a);
        #include <colorspace_fragment>
      }`});
    const mesh=new Mesh(new PlaneGeometry(1,1),material);mesh.visible=false;
    return {pass,material,mesh};
  },[]);
  useFrame(({gl})=>{
    const group=original.current;
    resources.mesh.visible=false;activeRef.current=false;
    if(group)group.visible=true;
    const frame=getFrame();
    if(!frame||!group)return;
    resources.pass.render(gl,frame.width,frame.height,frame.input);
    resources.mesh.scale.set(canvasWidth,canvasHeight,1);
    resources.mesh.visible=true;group.visible=false;activeRef.current=true;
  },-.5);
  useEffect(()=>()=>{
    if(original.current)original.current.visible=true;
    activeRef.current=false;
    resources.pass.dispose();resources.material.dispose();resources.mesh.geometry.dispose();
  },[resources,original,activeRef]);
  return <primitive object={resources.mesh} />;
}
