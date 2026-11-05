interface Task<T> {
  run: () => Promise<T>;
  resolve: (value: T | undefined) => void;
  reject: (error: unknown) => void;
}

export class LatestTaskQueue<T> {
  private running = false;
  private pending: Task<T> | null = null;

  run(task: () => Promise<T>): Promise<T | undefined> {
    this.clearPending();
    return new Promise((resolve, reject) => {
      this.pending = { run: task, resolve, reject };
      if (!this.running) void this.drain();
    });
  }

  clearPending() {
    this.pending?.resolve(undefined);
    this.pending = null;
  }

  private async drain() {
    this.running = true;
    while (this.pending) {
      const task = this.pending;
      this.pending = null;
      try {
        task.resolve(await task.run());
      } catch (error) {
        task.reject(error);
      }
    }
    this.running = false;
  }
}
