import {expect,it} from 'vitest';
import {prepareMaskAddition,prepareMaskEdit,type AddMaskInput} from './editing';
const input:AddMaskInput={sourceFilePath:'a',sourceTimeMs:1000,sourceDurationMs:10000,
 coordinateSpace:{x:200,y:100,width:400,height:200,sourceWidth:800,sourceHeight:400},type:'sensitive-data'};
const first=()=>prepareMaskAddition([],input,'one').mask;
it('creates a three-second source interval and focuses an existing disabled mask without changing it',()=>{
 const mask=first();expect(mask.sourceEndMs).toBe(4000);expect(mask.bounds).toEqual([{x:25,y:60,width:350,height:80}]);
 const disabled={...mask,disabled:true};
 expect(prepareMaskAddition([disabled],{...input,type:'highlight'},'two')).toEqual({mask:disabled,created:false});
});
it('limits creation to free time on the same recording, including exact minimum and endpoints',()=>{
 const mask=first();
 expect(prepareMaskAddition([mask],{...input,sourceTimeMs:500},'two').mask.sourceEndMs).toBe(1000);
 expect(()=>prepareMaskAddition([mask],{...input,sourceTimeMs:501},'two')).toThrow('Not enough space');
 expect(prepareMaskAddition([mask],{...input,sourceTimeMs:4000},'two').mask.sourceEndMs).toBe(7000);
 expect(prepareMaskAddition([mask],{...input,sourceFilePath:'b'},'two').created).toBe(true);
 expect(prepareMaskAddition([],{...input,sourceTimeMs:9500},'two').mask.sourceEndMs).toBe(10000);
 expect(()=>prepareMaskAddition([],{...input,sourceTimeMs:9501},'two')).toThrow();
 expect(()=>prepareMaskAddition([],{...input,sourceTimeMs:10000},'two')).toThrow();
});
it('retains geometry and opacity across crop changes and uses supplied drawn bounds',()=>{
 const mask={...first(),highlightOpacity:.7};
 const next={...input,sourceTimeMs:5000,coordinateSpace:{...input.coordinateSpace,x:0,y:0}};
 const created=prepareMaskAddition([mask],next,'two').mask;
 expect(created.bounds).toEqual([{x:225,y:160,width:350,height:80}]);expect(created.highlightOpacity).toBe(.7);
 expect(mask.bounds[0].x).toBe(25);
 const bounds=[{x:4,y:6,width:20,height:30}];
 expect(prepareMaskAddition([mask],{...next,bounds},'two').mask.bounds).toEqual(bounds);
});
it('validates edits and refuses overlapping timing without mutating input',()=>{
 const mask=first(),next={...first(),id:'two',sourceStartMs:5000,sourceEndMs:8000};
 expect(prepareMaskEdit([mask,next],'one',{sourceEndMs:5000})[0].sourceEndMs).toBe(5000);
 expect(()=>prepareMaskEdit([mask,next],'one',{sourceEndMs:5001})).toThrow('overlaps');
 expect(()=>prepareMaskEdit([mask],'one',{sourceEndMs:1499})).toThrow('half a second');
 expect(()=>prepareMaskEdit([mask],'one',{bounds:[]})).toThrow();
 expect(mask.sourceEndMs).toBe(4000);
 // Existing short retained intervals remain editable without changing their timing.
 expect(prepareMaskEdit([{...mask,sourceEndMs:1001}],'one',{disabled:true})[0].disabled).toBe(true);
});
