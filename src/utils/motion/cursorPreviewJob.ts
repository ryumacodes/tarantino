export class CursorPreviewJob {
  private id: string | null = null;
  private stopped = false;

  constructor(
    private begin: () => Promise<string>,
    private end: (id: string) => Promise<void>,
  ) {}

  async run(task: (id: string) => Promise<void>): Promise<void> {
    if (this.stopped) return;
    const id = await this.begin();
    this.id = id;
    try {
      if (!this.stopped) await task(id);
    } finally {
      if (this.id === id) {
        this.id = null;
        await this.end(id);
      }
    }
  }

  async cancel(): Promise<void> {
    this.stopped = true;
    const id = this.id;
    this.id = null;
    if (id !== null) await this.end(id);
  }
}
