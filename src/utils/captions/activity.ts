import type {CaptionGroup} from './groups';
/** Source-time activity window includes one second on either side of a group. */
export class CaptionActivity<T> {
  private ends:number[]=[];
  constructor(private groups:readonly CaptionGroup<T>[]) {
    let end=-Infinity,start=-Infinity;
    for(const group of groups) {
      if(!Number.isFinite(group.startMs)||!Number.isFinite(group.endMs)||group.startMs<start||group.endMs<group.startMs)
        throw new RangeError('Invalid caption group order');
      start=group.startMs;end=Math.max(end,group.endMs+1000);this.ends.push(end);
    }
  }
  at(time:number):number[] {
    if(!Number.isFinite(time))return [];
    // Mount/unmount decisions use whole-second source-time buckets.
    time=Math.round(time/1000)*1000;
    let low=0,high=this.ends.length;
    while(low<high){const mid=(low+high)>>>1;if(this.ends[mid]<time)low=mid+1;else high=mid;}
    const result:number[]=[];
    for(let i=low;i<this.groups.length&&this.groups[i].startMs-1000<=time;i++)
      if(this.groups[i].endMs+1000>=time)result.push(i);
    return result;
  }
}
