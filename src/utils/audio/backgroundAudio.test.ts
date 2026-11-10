import { expect,it,vi } from 'vitest';
import { createBackgroundAudioPlayer, exportBackgroundAudio, type BackgroundAudio } from './backgroundAudio';
const settings:BackgroundAudio={path:'/music.wav',name:'Music',volume:0.5,muted:false};
function media(){return {duration:2,currentTime:0,paused:true,readyState:1,volume:1,muted:false,loop:false,playbackRate:4,
  play:vi.fn(async function(this:any){this.paused=false;}),pause:vi.fn(function(this:any){this.paused=true;}),removeAttribute:vi.fn(),load:vi.fn(),addEventListener:vi.fn(),removeEventListener:vi.fn()} as unknown as HTMLAudioElement;}
it('loops on edited time and preserves natural music speed',async()=>{
 const audio=media(), player=createBackgroundAudioPlayer(audio,vi.fn());
 player.sync(5250,true,settings);await Promise.resolve();
 expect(audio.currentTime).toBe(1.25);expect(audio.playbackRate).toBe(1);expect(audio.loop).toBe(true);
 expect(audio.volume).toBe(0.5);expect(audio.play).toHaveBeenCalledTimes(1);
 player.sync(5500,true,settings);expect(audio.currentTime).toBe(1.5);
 player.sync(3000,false,{...settings,muted:true});expect(audio.currentTime).toBe(1);expect(audio.paused).toBe(true);expect(audio.muted).toBe(true);
 player.dispose();expect(audio.removeAttribute).toHaveBeenCalledWith('src');
});
it('does not seek across the loop boundary for normal small clock drift',()=>{
 const audio=media();audio.currentTime=1.99;Object.assign(audio,{paused:false});
 createBackgroundAudioPlayer(audio,vi.fn()).sync(2010,true,settings);
 expect(audio.currentTime).toBe(1.99);
});
it('does not repeatedly call blocked play, and retries only on a gesture',async()=>{
 const audio=media(), report=vi.fn();vi.mocked(audio.play).mockRejectedValue(new Error('gesture required'));
 const player=createBackgroundAudioPlayer(audio,report);
 player.sync(0,true,settings);await Promise.resolve();await Promise.resolve();
 player.sync(10,true,settings);expect(audio.play).toHaveBeenCalledTimes(1);expect(report).toHaveBeenCalledTimes(1);
 player.sync(10,true,settings,true);expect(audio.play).toHaveBeenCalledTimes(2);
 player.dispose();
});
it('pauses a delayed play resolution after the user stops playback',async()=>{
 const audio=media();let resolve!:()=>void;
 vi.mocked(audio.play).mockImplementation(()=>new Promise<void>(done=>{resolve=()=>{Object.assign(audio,{paused:false});done();};}));
 const player=createBackgroundAudioPlayer(audio,vi.fn());
 player.sync(0,true,settings);player.sync(0,false,settings);resolve();
 await Promise.resolve();await Promise.resolve();expect(audio.paused).toBe(true);
});
it('transports independent music settings and preserves the no-music default',()=>{
 expect(exportBackgroundAudio(settings)).toEqual({path:'/music.wav',volume:0.5,muted:false});
 expect(exportBackgroundAudio()).toBeNull();
});

it('reports media failures, reloads on explicit retry, and removes listeners on disposal',()=>{
 const audio=media(),report=vi.fn();
 const player=createBackgroundAudioPlayer(audio,report);
 const listeners=new Map(vi.mocked(audio.addEventListener).mock.calls.map(([name,callback])=>[name,callback as ()=>void]));
 Object.assign(audio,{error:{code:4}});
 listeners.get('error')!();
 expect(report.mock.calls[0][0].message).toContain('could not be loaded');
 player.sync(0,true,settings);expect(audio.play).not.toHaveBeenCalled();
 player.retry();expect(audio.load).toHaveBeenCalledTimes(1);
 listeners.get('playing')!();expect(report).toHaveBeenLastCalledWith(null);
 player.dispose();
 expect(audio.removeEventListener).toHaveBeenCalledWith('error',listeners.get('error'));
 expect(audio.removeEventListener).toHaveBeenCalledWith('playing',listeners.get('playing'));
 const calls=report.mock.calls.length;listeners.get('error')!();expect(report).toHaveBeenCalledTimes(calls);
});

it('honors small explicit seeks across music loops without chasing normal drift',()=>{
 const audio=media(),player=createBackgroundAudioPlayer(audio,vi.fn());
 Object.assign(audio,{paused:false});
 player.sync(1990,true,settings,false,1);
 expect(audio.currentTime).toBe(1.99);
 player.sync(2010,true,settings,false,1);
 expect(audio.currentTime).toBe(1.99);
 player.sync(2020,true,settings,false,2);
 expect(audio.currentTime).toBeCloseTo(.02);
 audio.currentTime=.03;
 player.sync(2040,true,settings,false,2);
 expect(audio.currentTime).toBe(.03);
 player.sync(1990,true,settings,false,3);
 expect(audio.currentTime).toBe(1.99);
});

it.each(['metadata','seeking'])('retains the latest music seek while waiting for %s',waiting=>{
 const audio=media(),player=createBackgroundAudioPlayer(audio,vi.fn());
 Object.assign(audio,{paused:false});
 player.sync(1000,true,settings,false,1);
 Object.assign(audio,waiting==='metadata'?{duration:NaN}:{seeking:true});
 player.sync(1020,true,settings,false,2);
 player.sync(1040,true,settings,false,3);
 expect(audio.currentTime).toBe(1);
 Object.assign(audio,{duration:2,seeking:false});
 player.sync(1040,true,settings,false,3);
 expect(audio.currentTime).toBe(1.04);
});
