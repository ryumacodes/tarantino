import type {TimedTranscriptText} from './timing';
export interface TranscriptWord extends TimedTranscriptText {id:string;type:'word'|'sound-label';isHidden?:boolean}
export interface TranscriptGenerator {type:'system';locale:string}
export interface TranscriptSession {id:string;sourceFilePath:string;offsetMs:number;words:TranscriptWord[];generator?:TranscriptGenerator}
const record=(value:unknown):value is Record<string,unknown>=>!!value&&typeof value==='object'&&!Array.isArray(value);
const identity=(value:unknown):value is string=>typeof value==='string'&&value.length>0&&value.length<=4096;
const time=(value:unknown):value is number=>typeof value==='number'&&Number.isFinite(value)&&value>=0;
/** Keep raw recognition timing in projects; derive display timing separately. */
export function parseTranscripts(input:unknown):TranscriptSession[] {
  if(input===undefined)return [];
  const invalid=()=>new Error('Invalid project: transcripts');
  if(!Array.isArray(input)||input.length>10000)throw invalid();
  const sessions=new Set<string>();let total=0;
  return input.map(session=>{
    if(!record(session)||!identity(session.id)||sessions.has(session.id)||!identity(session.sourceFilePath)
      ||!time(session.offsetMs)||!Array.isArray(session.words))throw invalid();
    sessions.add(session.id);total+=session.words.length;if(total>1_000_000)throw invalid();
    let generator:TranscriptGenerator|undefined;
    if(session.generator!==undefined){
      if(!record(session.generator)||session.generator.type!=='system'||typeof session.generator.locale!=='string'
        ||!session.generator.locale.trim()||session.generator.locale.length>128)throw invalid();
      generator={type:'system',locale:session.generator.locale};
    }
    const offsetMs=session.offsetMs;
    const ids=new Set<string>();
    const words=session.words.map(word=>{
      if(!record(word)||!identity(word.id)||ids.has(word.id)||typeof word.text!=='string'||word.text.length>10000
        ||!time(word.startMs)||!time(word.endMs)||word.endMs<word.startMs
        ||!Number.isFinite(word.endMs+offsetMs)
        ||(word.isHidden!==undefined&&typeof word.isHidden!=='boolean')
        ||(word.type!=='word'&&word.type!=='sound-label'))throw invalid();
      ids.add(word.id);
      return {id:word.id,text:word.text,startMs:word.startMs,endMs:word.endMs,type:word.type,
        ...(word.isHidden===undefined?{}:{isHidden:word.isHidden})} as TranscriptWord;
    });
    return {id:session.id,sourceFilePath:session.sourceFilePath,offsetMs:session.offsetMs,words,...(generator?{generator}:{})};
  });
}
