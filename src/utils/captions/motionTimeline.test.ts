import {expect,it} from 'vitest';
import {captionGroups} from './groups';
import {CaptionActivity} from './activity';
import {captionFrameProvider,createCaptionMotion} from './motionTimeline';
import type {CaptionMotionOptions} from './targets';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const clip=(id:string,startTime:number,sourceIn:number,duration:number,playbackRate=1,sourceFilePath='/a'):TimelineClip=>({
 id,name:id,type:'video',trackId:'v',startTime,sourceIn,duration,playbackRate,sourceFilePath,sourceOut:sourceIn+duration*playbackRate,enabled:true,locked:false});
const options:CaptionMotionOptions={wordsReveal:'line-by-line',lineGrowth:'entire-line',wordEntrance:'appear'};
const stage={x:0,y:0,width:1200,height:600},position={x:.5,y:.9};
const groups=captionGroups([{text:'one',startMs:1000,endMs:1200}],text=>({width:text.length*10,height:20}),500);
const source={groups,fontSize:40};
it('uses edited source time, trims, repeats and gaps',()=>{
 const state={duration:5000,tracks:[track],clips:[clip('fast',0,800,400,2),clip('repeat',1000,800,1600,.5)],trimStart:50,trimEnd:2000};
 const provider=captionFrameProvider(state,'/a',()=>source,options,stage,position);
 expect(provider(50)!.words[0].target.opacity).toBe(1);
 expect(provider(500)).toBeNull();
 expect(provider(1350)!.words[0].target.opacity).toBe(1);
 expect(provider(1950)).toBeNull();expect(provider(-1)).toBeNull();
 const motion=createCaptionMotion(state,'/a',()=>source,options,stage,position,60);
 expect(motion.sample(500)).toBeNull();expect(motion.sample(150)!.words[0].target.opacity).toBeGreaterThan(0);
});
it('resolves each recording separately and follows track visibility',()=>{
 const state={duration:5000,tracks:[track],clips:[clip('other',0,1000,500,1,'/b')],trimStart:0,trimEnd:500};
 const provider=captionFrameProvider(state,'/a',path=>path==='/a'?source:undefined,options,stage,position);
 expect(provider(10)).toBeNull();
 expect(captionFrameProvider({...state,tracks:[{...track,visible:false}]},'/a',()=>source,options,stage,position)(10)).toBeNull();
});
it('bounds word work to nearby groups in a long transcript',()=>{
 const many=Array.from({length:10000},(_,i)=>({...groups[0],startMs:i*4000,endMs:i*4000+500}));
 const index=new CaptionActivity(many);
 expect(index.at(20_000_100)).toEqual([5000]);
 expect(index.at(19_999_000)).toEqual([5000]);
 expect(index.at(20_001_499)).toEqual([5000]);expect(index.at(20_001_500)).toEqual([]);
 const state={duration:40_000_000,tracks:[],clips:[],trimStart:0,trimEnd:40_000_000};
 const prepared={groups:many,fontSize:40};
 const provider=captionFrameProvider(state,'/a',()=>prepared,options,stage,position);
 expect(provider(20_000_100)!.words).toHaveLength(1);
});
it('handles overlapping group ranges without losing a long-lived earlier group',()=>{
 const index=new CaptionActivity([{...groups[0],startMs:0,endMs:10000},{...groups[0],startMs:100,endMs:200}]);
 expect(index.at(5000)).toEqual([0]);
 expect(()=>new CaptionActivity([{...groups[0],startMs:200,endMs:300},{...groups[0],startMs:100,endMs:200}])).toThrow();
});
