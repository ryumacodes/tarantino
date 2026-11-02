import {expect,it} from 'vitest';
import type {TranscriptSession} from './document';
import type {TimelineClip,TimelineTrack} from '../../stores/editor/types';
import {prepareCaptionWords,prepareCaptionGroups} from './prepare';
const session:TranscriptSession={id:'session',sourceFilePath:'/a',offsetMs:1000,words:[
 {id:'one',text:'one',startMs:0,endMs:200,type:'word'},
 {id:'two',text:'two',startMs:300,endMs:500,type:'word',isHidden:true},
 {id:'sound',text:'[click]',startMs:600,endMs:800,type:'sound-label'},
 {id:'cut',text:'cut',startMs:900,endMs:1100,type:'word'},
]};
const track:TimelineTrack={id:'v',name:'Video',type:'video',clips:[],height:60,visible:true,muted:false,solo:false,locked:false,order:0};
const clip:TimelineClip={id:'clip',name:'Clip',type:'video',trackId:'v',startTime:0,duration:900,sourceIn:1000,sourceOut:1900,playbackRate:1,enabled:true,locked:false};
const state={duration:3000,tracks:[track],clips:[clip]};
it('filters hidden, sound-label and removed words before measuring a line',()=>{
 expect(prepareCaptionWords(state,'/a','/a',[session],false).map(w=>w.id)).toEqual(['one']);
 expect(prepareCaptionWords(state,'/a','/a',[session],true).map(w=>w.id)).toEqual(['one','sound']);
 const measured:string[]=[];
 prepareCaptionGroups(state,'/a','/a',[session],false,text=>{measured.push(text);return {width:10,height:20};},500);
 expect(measured).not.toContain('two');expect(measured).not.toContain('[click]');expect(measured).not.toContain('cut');
});
it('isolates sources and retains session identity and original timestamps',()=>{
 const other={...session,id:'other',sourceFilePath:'/b'};
 expect(prepareCaptionWords(state,'/a','/b',[other],true)).toEqual([]);
 const result=prepareCaptionWords(state,'/a','/a',[session,other],false);
 expect(result[0]).toMatchObject({sessionId:'session',startMs:1000,endMs:1200});
 expect(session.words[0].startMs).toBe(0);
});
it('uses any positive source overlap and excludes a boundary-only contact',()=>{
 const edit={...state,clips:[{...clip,sourceIn:1199,sourceOut:1201,duration:2}]};
 expect(prepareCaptionWords(edit,'/a','/a',[session],true).map(w=>w.id)).toEqual(['one']);
 expect(prepareCaptionWords({...edit,clips:[{...edit.clips[0],sourceIn:1200}]},'/a','/a',[session],true)).toEqual([]);
});
it('supports legacy timelines and hidden tracks',()=>{
 expect(prepareCaptionWords({...state,tracks:[],clips:[]},'/a','/a',[session],false).map(w=>w.id)).toEqual(['one','cut']);
 expect(prepareCaptionWords({...state,tracks:[{...track,visible:false}]},'/a','/a',[session],true)).toEqual([]);
});
