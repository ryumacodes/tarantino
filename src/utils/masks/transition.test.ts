import {expect,it} from 'vitest';
import {maskTransitionTarget,maskTransitionWindow,maskCoverageTarget} from './transition';
import type {RecordingMask} from './document';
const mask=(id:string,start:number,end:number,type:RecordingMask['type']='sensitive-data'):RecordingMask=>({
 id,sourceFilePath:'a',sourceStartMs:start,sourceEndMs:end,disabled:false,type,
 bounds:[{x:0,y:0,width:100,height:50}],highlightOpacity:.5,
});
it('prepares sensitive coverage 200ms early but never overrides an active sensitive region',()=>{
 const a=mask('a',1000,2000),b=mask('b',2100,3000);
 expect(maskTransitionTarget(799,null,null,a)).toBeNull();
 expect(maskTransitionTarget(800,null,null,a)).toBe(a);
 expect(maskTransitionTarget(1950,a,null,b)).toBe(a);
 expect(maskTransitionWindow(a,null,1000)).toBeNull();
 expect(maskCoverageTarget(a,2000)).toBe(1);expect(maskCoverageTarget(a,2000.001)).toBe(0);
});
it('bridges short gaps and snaps when the gap is 400ms or longer',()=>{
 const a=mask('a',1000,2000,'highlight'),b=mask('b',2300,3000,'highlight');
 expect(maskTransitionTarget(2050,null,a,b)).toBe(b);
 expect(maskTransitionWindow(b,a,2050)).toBe(500);
 const distant={...b,sourceStartMs:2400};
 expect(maskTransitionTarget(2250,null,a,distant)).toBeNull();
 expect(maskTransitionWindow(distant,a,2250)).toBeNull();
});
it('uses no highlight lead, retains its tail, and ignores disabled neighbours without skipping them',()=>{
 const a=mask('a',1000,2000,'highlight');
 expect(maskTransitionTarget(900,null,null,a)).toBeNull();
 expect(maskTransitionTarget(2200,null,a,null)).toBe(a);
 expect(maskTransitionTarget(2201,null,a,null)).toBeNull();
 expect(maskTransitionTarget(1900,{...a,disabled:true},null,null)).toBeNull();
 expect(maskTransitionWindow(mask('b',2100,3000),{...a,disabled:true},2050)).toBeNull();
 expect(maskCoverageTarget(null,1000)).toBe(0);
});
it('snaps sensitive motion throughout the active range, including both endpoints',()=>{
 const a=mask('a',0,1000),b=mask('b',1200,2000);
 expect(maskTransitionWindow(b,a,1100)).toBe(200);
 for(const time of [1200,1500,2000])expect(maskTransitionWindow(b,a,time)).toBeNull();
 expect(()=>maskTransitionTarget(NaN,null,null,null)).toThrow();
});
