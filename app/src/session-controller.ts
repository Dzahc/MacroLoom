import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  SESSION_TEXT,
  type SessionSnapshot,
  type SessionTransport,
} from './session-contract.ts';

/** Owns revision reconciliation and disposable subscriptions to native lifecycle; commands never simulate success. */
export class SessionController {
  private state: SessionSnapshot = INITIAL_SESSION;
  private listeners = new Set<() => void>();
  private generation = 0;
  private transport: SessionTransport;
  /** @param transport Native event/command adapter. Starts no input or subscriptions. */
  constructor(transport: SessionTransport) {
    this.transport = transport;
  }
  /** @returns The immutable native-derived snapshot, stable until a newer revision or transport error. */
  getSnapshot = (): SessionSnapshot => this.state;
  /** @param listener React/store listener. @returns Idempotent cleanup of this subscription only. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Removes this observer without affecting native input. */
    return () => {
      this.listeners.delete(listener);
    };
  };
  /** Subscribes before querying state. @returns Cleanup that also releases late registrations and ignores pending IPC. */
  connect(): () => void {
    const generation = ++this.generation;
    let unlisten: (() => void) | undefined;
    /** @param value Native state. @returns Nothing; abandoned owner deliveries cannot overwrite a remount. */
    const receive = (value: SessionSnapshot) => {
      if (generation === this.generation) this.receive(value);
    };
    void this.transport
      .listen(receive)
      .then(
        /** @param cleanup Event unsubscriber. @returns Listener-first startup state, or immediate cleanup after disposal. */
        async (cleanup) => {
          if (generation !== this.generation) {
            cleanup();
            return;
          }
          unlisten = cleanup;
          receive(await this.transport.invoke(SESSION_COMMAND.snapshot));
        },
      )
      .catch(
        /** @param reason Native subscription/query failure. @returns Nothing; reports only for the current owner. */
        (reason: unknown) => {
          if (generation === this.generation) this.fail(reason);
        },
      );
    /** Invalidates outstanding work before releasing an installed listener. */
    return () => {
      if (generation === this.generation) this.generation += 1;
      if (unlisten) unlisten();
    };
  }
  /** @param snapshot Native command/event state. @returns Nothing; late command responses cannot replace newer events. */
  receive(snapshot: SessionSnapshot): void {
    if (snapshot.revision <= this.state.revision) return;
    this.state = snapshot;
    this.notify();
  }
  /** @param command Typed native control name. @param args Optional external payload. @returns Completion after applying a current snapshot or reporting failure. */
  async request(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<void> {
    const generation = this.generation;
    try {
      const snapshot = await this.transport.invoke(command, args);
      if (generation === this.generation) this.receive(snapshot);
    } catch (reason) {
      if (generation === this.generation) this.fail(reason);
    }
  }
  /** @param reason Transport failure. @returns Nothing; preserves backend phase and displays actionable uncertainty. */
  private fail(reason: unknown): void {
    this.state = {
      ...this.state,
      message: `${SESSION_TEXT.failure}: ${String(reason)}`,
    };
    this.notify();
  }
  /** Notifies mounted observers; performs no native or timer work. */
  private notify(): void {
    this.listeners.forEach(
      /** @param listener Mounted store observer. @returns Nothing. */
      (listener) => listener(),
    );
  }
}
