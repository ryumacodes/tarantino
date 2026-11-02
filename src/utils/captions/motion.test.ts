import {expect,it} from 'vitest';
import {CaptionMotion,type CaptionFrameTargets} from './motion';
const target=(time:number,sourceKey='a'):CaptionFrameTargets=>({sourceKey,
 background:{x:time<500?0:100,y:0,width:time<500?20:100,height:20,opacity:time>=500&&time<1000?1:0},
 words:[{groupIndex:0,wordIndex:0,target:{x:time<500?0:100,y:0,width:30,height:20,opacity:time>=500&&time<1000?1:0}}]});
it('springs position/opacity while dimensions update immediately',()=>{
 const motion=new CaptionMotion(60,target);
 for(let i=0;i<=30;i++)motion.sample(i*1000/60);
 const frame=motion.sample(500)!;
 expect(frame.background.width).toBe(100);expect(frame.background.x).toBeGreaterThan(0);expect(frame.background.x).toBeLessThan(100);
 expect(frame.background.opacity).toBeGreaterThan(0);expect(frame.background.opacity).toBeLessThan(1);
 expect(frame.background.blurRadius).toBeCloseTo(10*(1-frame.background.opacity));
 expect(motion.sample(2000)!.background.opacity).toBe(0);
});
it('interpolates animated values but samples immediate dimensions at exact time',()=>{
 const motion=new CaptionMotion(60,time=>{const frame=target(time);frame.background.width=time;return frame;});
 const a=motion.sample(500)!,b=motion.sample(500+1000/60)!,middle=motion.sample(500+500/60)!;
 expect(middle.background.x).toBeCloseTo((a.background.x+b.background.x)/2);
 expect(middle.background.width).toBe(500+500/60);
 a.background.x=999;expect(motion.sample(500)!.background.x).not.toBe(999);
});
it('bounds seek reconstruction and produces deterministic backward seeks',()=>{
 let calls=0;const motion=new CaptionMotion(60,time=>{calls++;return target(time);});
 motion.sample(36_000_000);expect(calls).toBeLessThanOrEqual(63);
 expect(motion.sample(550)).toEqual(new CaptionMotion(60,target).sample(550));
});
it('does not mix springs across sources or show captions inside a gap',()=>{
 const motion=new CaptionMotion(60,time=>time>=600&&time<700?null:target(time,time>=700?'b':'a'));
 motion.sample(550);expect(motion.sample(650)).toBeNull();
 const switched=motion.sample(700)!;expect(switched.background.opacity).toBe(1);
 expect(switched.background.x).toBe(100);
});
