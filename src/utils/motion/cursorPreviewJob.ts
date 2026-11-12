export class CursorPreviewJob {
  private id: string | null = null;
  private stopped = false;

  constructor(
    private begin: () => Promise<string>,
    private end: (id: string) => Promise<void>,
  ) {}

  async replace(): Promise<string | null> {
    if (this.stopped) return null;
    const id = await this.begin();
    const previous = this.id;
    this.id = null;
    if (previous !== null) await this.end(previous);
    if (this.stopped) {
      await this.end(id);
      return null;
    }
    this.id = id;
    return id;
  }

  async run(task: (id: string) => Promise<void>): Promise<void> {
    const id = await this.replace();
    if (id === null || this.stopped) return;
    try {
      await task(id);
    } finally {
      const current = this.id;
      this.id = null;
      if (current !== null) await this.end(current);
    }
  }

  async cancel(): Promise<void> {
    this.stopped = true;
    const id = this.id;
    this.id = null;
    if (id !== null) await this.end(id);
  }
}
