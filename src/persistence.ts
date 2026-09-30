/** Serial, optimistic preferences. Patches merge synchronously, before React re-renders. */
export class Preferences<T extends object> {
  current: T;
  private tail: Promise<void> = Promise.resolve();
  private dirty = false;
  get pending() {
    return this.dirty;
  }
  private write: (value: T) => Promise<unknown>;

  constructor(initial: T, write: (value: T) => Promise<unknown>) {
    this.current = initial;
    this.write = write;
  }
  /** Replaces state with an already persisted value. */
  restore(value: T) {
    this.current = value;
    this.dirty = false;
  }
  /** Merges immediately and appends an immutable value to the write queue. */
  patch(patch: Partial<T>): Promise<void> {
    this.current = { ...this.current, ...patch };
    const value = this.current;
    this.dirty = true;
    const task = this.tail.then(async () => {
      await this.write(value);
      if (value === this.current) this.dirty = false;
    });
    // A failed write must not poison subsequent writes, or roll back a newer UI value.
    this.tail = task.catch(() => {});
    return task;
  }
  /** Drains writes and retries the latest dirty value after a failure. */
  async flush(): Promise<void> {
    await this.tail;
    if (this.dirty) await this.patch({});
  }
}

/** Keep captured bytes for retry. Session-changing commands must await flush(). */
export class PendingCapture {
  private running: Promise<void> = Promise.resolve();
  private retry: (() => Promise<void>) | null = null;
  get pending() {
    return this.retry !== null;
  }
  /** Captures bytes once and retains a save closure until persistence succeeds. */
  start(capture: () => Promise<string>, save: (png: string) => Promise<unknown>): Promise<void> {
    if (this.pending) throw new Error('Предыдущий снимок ещё не сохранён');
    const bytes = capture();
    this.retry = async () => {
      let png: string;
      try {
        png = await bytes;
      } catch (error) {
        this.retry = null;
        throw error;
      } // No image was produced.
      await save(png);
      this.retry = null;
    };
    const task = this.retry();
    this.running = task.catch(() => {});
    return task;
  }
  /** Waits for capture/save and retries the same bytes when necessary. */
  async flush(): Promise<void> {
    await this.running;
    if (this.retry) await this.retry();
  }
}
