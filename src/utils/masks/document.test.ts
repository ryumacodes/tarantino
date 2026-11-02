import {describe,it,expect} from 'vitest';
import {parseMasks,initialMaskBounds,type RecordingMask} from './document';
const mask:RecordingMask={id:'mask',sourceFilePath:'/recording.mp4',sourceStartMs:1000,sourceEndMs:4000,
 disabled:false,type:'sensitive-data',bounds:[{x:10,y:20,width:100,height:50}],highlightOpacity:.5};
describe('mask documents',()=>{
 it('reads legacy absence and preserves multiple regions without sharing mutable bounds',()=>{
  expect(parseMasks(undefined)).toEqual([]);
  const input={...mask,bounds:[...mask.bounds,{x:-10,y:0,width:20,height:20}]};
  const parsed=parseMasks([input]);expect(parsed).toEqual([input]);parsed[0].bounds[0].x=55;expect(mask.bounds[0].x).toBe(10);
 });
 it('rejects invalid privacy coverage rather than discarding it',()=>{
  for(const patch of [{sourceStartMs:-1},{sourceEndMs:1000},{sourceEndMs:Infinity},{disabled:1},
   {type:'blur'},{highlightOpacity:0},{highlightOpacity:1},{bounds:[]},{sourceFilePath:''},
   {bounds:[{x:0,y:0,width:0,height:1}]},{bounds:[{x:0,y:NaN,width:1,height:1}]},
   {bounds:[{x:Number.MAX_VALUE,y:0,width:Number.MAX_VALUE,height:1}]}])
    expect(()=>parseMasks([{...mask,...patch}])).toThrow('Invalid project: masks');
  expect(()=>parseMasks([mask,mask])).toThrow();
 });
 it('allows short retained intervals after edits without imposing creation minimums',()=>{
  expect(parseMasks([{...mask,sourceEndMs:1001}])[0].sourceEndMs).toBe(1001);
 });
 it('centers default bounds inside small and large recordings and copies previous bounds',()=>{
  expect(initialMaskBounds(1000,600)).toEqual([{x:325,y:260,width:350,height:80}]);
  expect(initialMaskBounds(100,40)).toEqual([{x:0,y:0,width:100,height:40}]);
  const previous=initialMaskBounds(1000,600,mask.bounds);expect(previous).toEqual(mask.bounds);expect(previous[0]).not.toBe(mask.bounds[0]);
  expect(()=>initialMaskBounds(0,600)).toThrow();
 });
});
it('copies and validates the creation coordinate space',()=>{
 const coordinateSpace={x:200,y:100,width:400,height:200,sourceWidth:800,sourceHeight:400};
 const input={...mask,coordinateSpace};
 const parsed=parseMasks([input])[0];expect(parsed).toEqual(input);
 parsed.coordinateSpace!.x=0;expect(coordinateSpace.x).toBe(200);
 for(const patch of [{x:-1},{width:0},{sourceWidth:0},{x:401},{y:201},{height:.5},{sourceHeight:Infinity}])
  expect(()=>parseMasks([{...input,coordinateSpace:{...coordinateSpace,...patch}}])).toThrow('coordinate space');
});
