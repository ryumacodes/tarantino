/** One running calculation and only the newest queued edit per preview consumer. */
export function createLatestTaskQueue() {
  type Job = { cancelled: boolean; start: () => Promise<void> };
  let running: Job | undefined;
  let pending: Job | undefined;
  const pump = () => {
    if (running || !pending) return;
    const job = pending;
    pending = undefined;
    running = job;
    void job.start().finally(() => {
      running = undefined;
      pump();
    });
  };
  return {
    submit<T>(run: () => Promise<T>, success: (value: T) => void, failure: (error: unknown) => void) {
      const job: Job = {
        cancelled: false,
        async start() {
          try {
            const value = await run();
            if (!job.cancelled) success(value);
          } catch (error) {
            if (!job.cancelled) failure(error);
          }
        },
      };
      if (pending) pending.cancelled = true;
      pending = job;
      pump();
      return () => {
        job.cancelled = true;
        if (pending === job) pending = undefined;
      };
    },
  };
}
