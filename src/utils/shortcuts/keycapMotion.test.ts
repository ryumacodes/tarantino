import {shortcutLayout,shortcutLabels} from './keycaps';
import {expect,it} from 'vitest';
import {keycapInstances} from './keycapInstances';
import {shortcutIntervals} from './intervals';
import {KeycapMotion,keycapMotionTarget} from './keycapMotion';
import {DEFAULT_SHORTCUT_SETTINGS} from './settings';
const instances=keycapInstances(shortcutIntervals([{timestamp:1000,type:'down',key:'c',modifiers:['command']} ]));
const settings=DEFAULT_SHORTCUT_SETTINGS;
it('staggered keys spring toward visible targets and fade after their source interval',()=>{
 const motion=new KeycapMotion(instances,settings,1000,600,60,time=>({time}));
 expect(motion.sample(800).map(c=>c.opacity)).toEqual([0,0]);
 const entering=motion.sample(1050);
 expect(entering[0].opacity).toBeGreaterThan(entering[1].opacity);
 expect(entering[0].opacity).toBeLessThan(1);
 expect(motion.sample(2000).map(c=>c.opacity)).toEqual([1,1]);
 expect(motion.sample(3500).map(c=>c.opacity)).toEqual([0,0]);
});
it('interpolates fractional frames and protects cached results from mutation',()=>{
 const motion=new KeycapMotion(instances,settings,1000,600,60,time=>({time}));
 const a=motion.sample(1000),b=motion.sample(1000+1000/60),middle=motion.sample(1000+500/60);
 expect(middle[0].opacity).toBeCloseTo((a[0].opacity+b[0].opacity)/2,10);
 a[0].opacity=999;expect(motion.sample(1000)[0].opacity).not.toBe(999);
});
it('samples the output mapping, suppressing gaps and fullscreen camera targets',()=>{
 expect(keycapMotionTarget(instances[0],{time:1500,fullscreenCamera:true},settings,1000,600).opacity).toBe(0);
 expect(keycapMotionTarget(instances[0],null,settings,1000,600).opacity).toBe(0);
 const seen:number[]=[];
 const motion=new KeycapMotion(instances,settings,1000,600,60,time=>{seen.push(time);return {time:time*2};});
 expect(motion.sample(800)[0].opacity).toBeGreaterThan(.9);
 expect(Math.max(...seen)).toBe(800);
});
it('bounds reconstruction work for distant and backward seeks',()=>{
 let calls=0;
 const motion=new KeycapMotion(instances,settings,1000,600,60,time=>{calls++;return {time};});
 expect(motion.sample(36_000_000).map(c=>c.opacity)).toEqual([0,0]);
 expect(calls).toBeLessThanOrEqual(62);
 calls=0;
 const backward=motion.sample(1050);
 expect(calls).toBeLessThanOrEqual(62);
 const fresh=new KeycapMotion(instances,settings,1000,600,60,time=>({time}));
 expect(backward).toEqual(fresh.sample(1050));
});
it('cached chord geometry matches direct layout across handoffs and canvas sizes',()=>{
 const shared=keycapInstances(shortcutIntervals([
  {timestamp:500,key:'c',modifiers:['command'],type:'down'},
  {timestamp:1100,key:'v',modifiers:['shift','command'],type:'down'},
 ]));
 for(const [width,height] of [[320,900],[1920,1080],[500,500]]) {
  const layouts=new Map();
  for(const instance of shared)for(const segment of instance.segments)
   layouts.set(segment.interval,shortcutLayout(shortcutLabels(segment.interval),settings,width,height));
  for(const time of [0,400,500,1000,1100,2000,3000])for(const instance of shared)
   expect(keycapMotionTarget(instance,{time},settings,width,height,layouts))
    .toEqual(keycapMotionTarget(instance,{time},settings,width,height));
 }
});
