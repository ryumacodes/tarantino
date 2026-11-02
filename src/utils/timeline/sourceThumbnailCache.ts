export interface SourceThumbnail { source_time_ms: number; image: string; }
type Loader = (times: number[]) => Promise<SourceThumbnail[]>;

/** Small per-recording cache; pending requests are shared and failed samples do not loop. */
export class SourceThumbnailCache {
  private images = new Map<number,string>();
  private pending = new Map<number,Promise<void>>();
  private failed = new Set<number>();
  constructor(private load: Loader, private limit = 192) {}
  key(time: number) { return Math.round(time*1000)/1000; }
  get(time: number) { return this.images.get(this.key(time)); }
  async ensure(times: number[]) {
    const keys = [...new Set(times.filter(time => Number.isFinite(time) && time>=0).map(time=>this.key(time)))];
    const missing = keys.filter(key=>!this.images.has(key) && !this.pending.has(key) && !this.failed.has(key)).slice(0,16);
    if (missing.length) {
      const task = Promise.resolve().then(()=>this.load(missing)).then(samples=>{
        for (const sample of samples) {
          const key=this.key(sample.source_time_ms);
          if (missing.includes(key)) this.images.set(key,sample.image);
        }
        for (const key of missing) if (!this.images.has(key)) this.failed.add(key);
        while (this.images.size>this.limit) this.images.delete(this.images.keys().next().value!);
      }).catch(()=>{ for (const key of missing) this.failed.add(key); }).finally(()=>{
        for (const key of missing) this.pending.delete(key);
        while (this.failed.size>this.limit) this.failed.delete(this.failed.values().next().value!);
      });
      for (const key of missing) this.pending.set(key,task);
    }
    await Promise.all(keys.map(key=>this.pending.get(key)));
  }
}
