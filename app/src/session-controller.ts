import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  SESSION_TEXT,
  type SessionSnapshot,
  type SessionTransport,
  type SnapshotCommand,
} from './session-contract.ts';

/** Owns revision reconciliation and disposable subscriptions to native lifecycle; commands never simulate success. */
export class SessionController {
  private state: SessionSnapshot = INITIAL_SESSION;
  private listeners = new Set<() => void>();
  private generation = 0;
  private transport: SessionTransport;
  private owner = crypto.randomUUID();
  private claims = new Map<string, number>();
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
    const owner = crypto.randomUUID();
    this.owner = owner;
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
      this.releaseGeneration(generation);
    };
  }
  /** @param snapshot Native command/event state. @returns Nothing; late command responses cannot replace newer events. */
  receive(snapshot: SessionSnapshot): void {
    if (snapshot.revision <= this.state.revision) return;
    this.state = snapshot;
    this.notify();
  }
  /** @param command Native command with empty input and a snapshot result. @returns Completion after applying current native state or reporting failure. */
  async request(command: SnapshotCommand): Promise<void> {
    const generation = this.generation;
    try {
      const snapshot = await this.transport.invoke(command);
      if (generation === this.generation) this.receive(snapshot);
    } catch (reason) {
      if (generation === this.generation) this.fail(reason);
    }
  }
  /** @returns The current native confirmation owner, also supplied for atomic handoff to a confirmed deletion. */
  get confirmationOwner(): string {
    return this.owner;
  }
  /** @param blocked Whether to claim or release this mounted owner's confirmation. @returns Success only if the owner remains mounted and the native claim succeeds. */
  async setConfirmation(blocked: boolean): Promise<boolean> {
    const generation = this.generation;
    if (blocked) this.owner = crypto.randomUUID();
    const owner = this.owner;
    const confirmation = this.transport.confirmation;
    if (!confirmation) return false;
    if (!blocked && !this.claims.has(owner)) return true;
    if (blocked) this.claims.set(owner, generation);
    try {
      const snapshot = await confirmation({ owner, blocked });
      if (generation !== this.generation) {
        if (blocked) this.releaseOwner(owner);
        return false;
      }
      if (!blocked) this.claims.delete(owner);
      this.receive(snapshot);
      return true;
    } catch (reason) {
      this.confirmationFailure(owner, blocked, generation, reason);
      return false;
    }
  }
  /** @param owner Failed claim identity. @param blocked Whether native acquisition may have succeeded before acknowledgement failed. @param generation Initiating mount. @param reason Native failure. @returns Nothing; releases uncertain claims and reports only to the current owner. */
  private confirmationFailure(
    owner: string,
    blocked: boolean,
    generation: number,
    reason: unknown,
  ): void {
    if (blocked) this.releaseOwner(owner);
    if (generation === this.generation) this.fail(reason);
  }
  /** @param owner Abandoned owner identity. @returns Nothing; idempotently releases current or late native claims without releasing another owner. */
  private releaseOwner(owner: string): void {
    const generation = this.generation;
    const confirmation = this.transport.confirmation;
    if (!confirmation) return;
    void confirmation({ owner, blocked: false })
      .then(
        /** Deletes only the acknowledged claim; another dialog uses a different identity. */
        () => {
          this.claims.delete(owner);
        },
      )
      .catch(
        /** @param reason Native ownership release failure. @returns Nothing; reports a transport failure without silently swallowing it. */
        (reason: unknown) => {
          if (generation === this.generation) this.fail(reason);
        },
      );
  }
  /** @param generation Disconnected owner generation. @returns Nothing; leaves claims created by a remount untouched. */
  private releaseGeneration(generation: number): void {
    for (const [owner, claimedGeneration] of this.claims) {
      if (claimedGeneration === generation) this.releaseOwner(owner);
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
