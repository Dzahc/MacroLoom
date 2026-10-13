import {
  INITIAL_SESSION,
  SESSION_COMMAND,
  SESSION_TEXT,
  type SessionSnapshot,
  type SessionTransport,
  type SnapshotCommand,
} from './session-contract.ts';

/** One native claim keeps cancellation intent until its potentially delayed acquisition settles. */
type ConfirmationClaim = {
  owner: string;
  generation: number;
  pending: boolean;
  cancelled: boolean;
};

/** Owns revision reconciliation and disposable subscriptions to native lifecycle; commands never simulate success. */
export class SessionController {
  private state: SessionSnapshot = INITIAL_SESSION;
  private listeners = new Set<() => void>();
  private generation = 0;
  private transport: SessionTransport;
  private owner = crypto.randomUUID();
  private claims = new Map<string, ConfirmationClaim>();
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
  /** @param owner Completed deletion's original confirmation identity. @returns Nothing; finishes client ownership and releases any residual native claim after handoff/error. */
  confirmationDone(owner: string): void {
    this.releaseOwner(owner);
  }
  /** @param blocked Whether to claim or release this mounted owner's confirmation. @returns Success only if the owner remains mounted and the native claim succeeds. */
  async setConfirmation(blocked: boolean): Promise<boolean> {
    const confirmation = this.transport.confirmation;
    if (!confirmation) return false;
    const claim = this.prepareClaim(blocked);
    if (!claim) return true;
    try {
      const snapshot = await confirmation({ owner: claim.owner, blocked });
      return this.confirmationAccepted(claim, blocked, snapshot);
    } catch (reason) {
      this.confirmationFailure(claim, blocked, reason);
      return false;
    }
  }
  /** @param blocked Acquisition or cancellation. @returns A per-dialog claim, preserving cancellation until pending acquisition settles. */
  private prepareClaim(blocked: boolean): ConfirmationClaim | undefined {
    if (blocked) {
      this.owner = crypto.randomUUID();
      const claim = {
        owner: this.owner,
        generation: this.generation,
        pending: true,
        cancelled: false,
      };
      this.claims.set(this.owner, claim);
      return claim;
    }
    const claim = this.claims.get(this.owner);
    if (claim) claim.cancelled = true;
    return claim;
  }
  /** @param claim Owner of this receipt. @param blocked Whether it acquired a native reservation. @param snapshot Revisioned receipt. @returns Whether the current live owner may use the successful result. */
  private confirmationAccepted(
    claim: ConfirmationClaim,
    blocked: boolean,
    snapshot: SessionSnapshot,
  ): boolean {
    if (blocked) claim.pending = false;
    if (claim.generation !== this.generation || (blocked && claim.cancelled)) {
      this.releaseOwner(claim.owner);
      return false;
    }
    if (!blocked && !claim.pending) this.claims.delete(claim.owner);
    this.receive(snapshot);
    return true;
  }
  /** @param claim Failed native transaction owner. @param blocked Whether acquisition may have succeeded before acknowledgement failed. @param reason Native failure. @returns Nothing; releases uncertain claims and reports only to the current dialog. */
  private confirmationFailure(
    claim: ConfirmationClaim,
    blocked: boolean,
    reason: unknown,
  ): void {
    if (blocked) {
      claim.pending = false;
      this.releaseOwner(claim.owner);
    }
    if (claim.generation === this.generation && claim.owner === this.owner)
      this.fail(reason);
  }
  /** @param owner Abandoned claim identity. @returns Nothing; cancellation intent survives an early release until delayed acquisition is acknowledged. */
  private releaseOwner(owner: string): void {
    const generation = this.generation;
    const confirmation = this.transport.confirmation;
    if (!confirmation) return;
    const claim = this.claims.get(owner);
    if (claim) claim.cancelled = true;
    void confirmation({ owner, blocked: false })
      .then(
        /** Retains a cancelled claim while acquisition is pending; its later receipt triggers another release. */
        () => {
          if (!claim?.pending) this.claims.delete(owner);
        },
      )
      .catch(
        /** @param reason Native ownership release failure. @returns Nothing; reports only to this current dialog owner. */
        (reason: unknown) => {
          if (generation === this.generation && owner === this.owner)
            this.fail(reason);
        },
      );
  }
  /** @param generation Disconnected owner generation. @returns Nothing; leaves claims created by a remount untouched. */
  private releaseGeneration(generation: number): void {
    for (const [owner, claim] of this.claims) {
      if (claim.generation === generation) this.releaseOwner(owner);
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
