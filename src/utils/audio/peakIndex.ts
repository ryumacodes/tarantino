const BLOCK_SIZE=64;
const cache=new WeakMap<readonly number[],PeakIndex>();

/** Immutable peak arrays share compact block maxima; partial blocks remain exact. */
export class PeakIndex {
  private readonly leafCount:number;
  private readonly tree:Float64Array;
  constructor(private readonly peaks:readonly number[]){
    this.leafCount=2**Math.ceil(Math.log2(Math.max(1,Math.ceil(peaks.length/BLOCK_SIZE))));
    this.tree=new Float64Array(this.leafCount*2);
    for(let i=0;i<peaks.length;i++){
      const node=this.leafCount+Math.floor(i/BLOCK_SIZE);
      this.tree[node]=Math.max(this.tree[node],peaks[i]);
    }
    for(let node=this.leafCount-1;node>0;node--)this.tree[node]=Math.max(this.tree[node*2],this.tree[node*2+1]);
  }
  get maximum(){return this.tree[1];}
  get byteLength(){return this.tree.byteLength;}
  /** Half-open sample index range, including every intersecting sample bin. */
  range(start:number,end:number){
    if(!Number.isFinite(start) || !Number.isFinite(end))return 0;
    let left=Math.max(0,Math.floor(start)),right=Math.min(this.peaks.length,Math.ceil(end));
    let maximum=0;
    while(left<right && left%BLOCK_SIZE!==0)maximum=Math.max(maximum,this.peaks[left++]);
    while(left<right && right%BLOCK_SIZE!==0)maximum=Math.max(maximum,this.peaks[--right]);
    left=this.leafCount+left/BLOCK_SIZE;right=this.leafCount+right/BLOCK_SIZE;
    while(left<right){
      if(left%2===1)maximum=Math.max(maximum,this.tree[left++]);
      if(right%2===1)maximum=Math.max(maximum,this.tree[--right]);
      left=Math.floor(left/2);right=Math.floor(right/2);
    }
    return maximum;
  }
}
export function peakIndex(peaks:readonly number[]){
  let index=cache.get(peaks);
  if(!index){index=new PeakIndex(peaks);cache.set(peaks,index);}
  return index;
}
