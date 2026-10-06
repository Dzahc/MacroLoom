import {
  LABEL,
  PHASE,
  reconcileSelection,
  requestAction,
  canRequest,
  ACTION,
  type LibraryAction,
  type LibrarySnapshot,
  type SelectionChange,
  type MacroSummary,
} from './library-model.ts';
import type { BackendLibraryState, MacroDocument } from './macro-contract.ts';
import { OutcomeQueue } from './outcome-queue.ts';

export type LibraryTransport = {
  listen: (
    receive: (state: BackendLibraryState) => void,
  ) => Promise<() => void>;
  load: () => Promise<BackendLibraryState>;
  read: (id: string) => Promise<MacroDocument>;
  delete: (id: string) => Promise<BackendLibraryState>;
};
export type PreparedAction = Readonly<{
  request: LibraryAction;
  snapshot: MacroDocument;
}>;
const LOAD_ERROR = 'Could not load macros';
const ACTION_ERROR = 'Could not prepare macro';
const FILE_GUIDANCE =
  'Correct the file and restart MacroLoom. File left unchanged.';
const INITIAL_REVISION = -1;
const DELETE_MESSAGE = {
  pending: 'Deleting',
  success: 'Deleted',
  failure: 'Could not delete',
} as const;

/**
 * Owns a backend-derived library store, stable selection and once-per-failure notifications.
 * Event and command snapshots reconcile by revision; no events are retained in library state.
 */
export class LibraryController {
  readonly queue = new OutcomeQueue();
  private transport: LibraryTransport;
  private listeners = new Set<() => void>();
  private revision = INITIAL_REVISION;
  private seenFailures = 0;
  private nextOutcome = 0;
  private generation = 0;
  private pendingDelete: MacroSummary | null = null;
  private latestBackend: BackendLibraryState | null = null;
  private state: LibrarySnapshot = {
    macros: [],
    selectedId: null,
    phase: PHASE.idle,
    message: LABEL.loading,
    loading: true,
    writable: false,
  };

  /** @param transport Native command/event boundary; failures become user-visible notifications. */
  constructor(transport: LibraryTransport) {
    this.transport = transport;
  }
  /** @returns Immutable presentation state, stable between meaningful updates. */
  getSnapshot = (): LibrarySnapshot => this.state;
  /** @param listener Store subscriber. @returns Idempotent unsubscribe with no disk side effects. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Removes the owner listener without affecting native discovery. */
    return () => {
      this.listeners.delete(listener);
    };
  };

  /**
   * Subscribes before starting discovery so progressive events cannot be missed.
   * @returns Cleanup suppressing pending callbacks and releasing late listener registrations.
   * Backend discovery remains idempotent across React StrictMode cleanup/remount.
   */
  connect(): () => void {
    this.generation += 1;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    /** @param state Revisioned backend state, ignored after disconnect. */
    const receive = (state: BackendLibraryState) => {
      if (!disposed) this.apply(state);
    };
    void this.transport
      .listen(receive)
      .then(
        /** @param cleanup Native listener cleanup. @returns Discovery completion, or nothing after disposal. */
        async (cleanup) => {
          if (disposed) {
            cleanup();
            return;
          }
          unlisten = cleanup;
          receive(await this.transport.load());
        },
      )
      .catch(
        /** @param reason Subscription/startup failure, ignored after owner disconnect. */
        (reason: unknown) => {
          if (!disposed) this.failLoad(reason);
        },
      );
    /** Suppresses in-flight work and releases the registered listener, when available. */
    return () => {
      disposed = true;
      this.generation += 1;
      this.pendingDelete = null;
      if (unlisten) unlisten();
    };
  }

  /** @param change Clicked ID/source. @returns Nothing; only present idle entries may become selected. */
  select(change: SelectionChange): void {
    if (this.state.phase !== PHASE.idle) return;
    if (reconcileSelection(this.state.macros, change.macroId) === null) return;
    this.state = { ...this.state, selectedId: change.macroId };
    this.notify();
  }

  /**
   * Obtains an action-owned validated snapshot lazily; later stories consume the returned request/data.
   * @param request Typed clicked-ID operation. @returns Prepared data, or null for unavailable/non-macro actions.
   * External-file failures produce a persistent toast; this does not save, delete or inject input.
   */
  async prepare(request: LibraryAction): Promise<PreparedAction | null> {
    if (!('macroId' in request)) return null;
    if (
      !requestAction(
        this.state,
        request.action,
        request.source,
        request.macroId,
      )
    )
      return null;
    const generation = this.generation;
    try {
      const snapshot = await this.transport.read(request.macroId);
      if (generation !== this.generation) return null;
      return { request, snapshot };
    } catch (reason) {
      if (generation !== this.generation) return null;
      this.failure(`${ACTION_ERROR}: ${String(reason)}`);
      return null;
    }
  }

  /**
   * Deletes a deliberately confirmed ID through the native boundary, without loading its contents.
   * @param macroId Frozen confirmation identity, independently checked against current availability.
   * @returns Completion after reporting a timed outcome; repeat requests are ignored while pending.
   * Backend snapshots own removal; failures retain entries and selection. Disconnect suppresses delivery.
   */
  async deleteConfirmed(macroId: string): Promise<void> {
    if (!canRequest(this.state, ACTION.delete, macroId)) return;
    const target = this.state.macros.find(
      /** @param macro Cached metadata. @returns Whether it is the confirmed target. */
      (macro) => macro.id === macroId,
    );
    if (!target) return;
    const generation = this.generation;
    this.pendingDelete = target;
    this.state = {
      ...this.state,
      deleting: macroId,
      message: `${DELETE_MESSAGE.pending} “${target.name}”…`,
    };
    this.notify();
    try {
      const result = await this.transport.delete(macroId);
      if (generation !== this.generation) return;
      this.apply(result);
      this.pendingDelete = null;
      this.apply(this.latestBackend ?? result);
      this.deleteOutcome(
        'success',
        `${DELETE_MESSAGE.success} “${target.name}”`,
      );
    } catch (reason) {
      if (generation !== this.generation) return;
      this.pendingDelete = null;
      this.state = { ...this.state, deleting: null, message: LABEL.ready };
      this.notify();
      this.deleteOutcome(
        'failure',
        `${DELETE_MESSAGE.failure} “${target.name}”: ${String(reason)}`,
      );
    }
  }

  /** @param kind Disk outcome. @param message Accessible action/name feedback. @returns Nothing; uses the ordinary toast timeout. */
  private deleteOutcome(kind: 'success' | 'failure', message: string): void {
    this.nextOutcome += 1;
    this.queue.enqueue({ id: this.nextOutcome, kind, message });
  }

  /** Cancels owned toast timers and resets failure delivery for a StrictMode reconnection. */
  dispose(): void {
    this.queue.dispose();
    this.seenFailures = 0;
  }

  /** @param incoming Backend snapshot. @returns Nothing; stale results cannot rewind loading or selection. */
  private apply(incoming: BackendLibraryState): void {
    if (incoming.revision < this.revision) return;
    this.revision = incoming.revision;
    this.latestBackend = incoming;
    const deleting = this.pendingDelete?.id ?? incoming.deleting;
    const target =
      this.pendingDelete ??
      incoming.macros.find(
        /** @param macro Backend metadata. @returns Whether it is being deleted. */
        (macro) => macro.id === deleting,
      );
    this.state = {
      ...this.state,
      macros: incoming.macros,
      loading: incoming.loading,
      writable: incoming.writable,
      deleting,
      message:
        target && deleting
          ? `${DELETE_MESSAGE.pending} “${target.name}”…`
          : incoming.loading
            ? LABEL.loading
            : LABEL.ready,
      selectedId: reconcileSelection(incoming.macros, this.state.selectedId),
    };
    incoming.failures.slice(this.seenFailures).forEach(
      /** @param failure New filename/field diagnostic. @returns Nothing; queues one persistent message. */
      (failure) =>
        this.failure(
          `${failure.file} — ${failure.field}: ${failure.message}. ${FILE_GUIDANCE}`,
        ),
    );
    this.seenFailures = incoming.failures.length;
    this.notify();
  }
  /** @param reason Load/subscribe failure. @returns Nothing; ends loading without silently changing storage. */
  private failLoad(reason: unknown): void {
    this.state = {
      ...this.state,
      loading: false,
      writable: false,
      message: `${LOAD_ERROR}: ${String(reason)}`,
    };
    this.failure(this.state.message);
    this.notify();
  }
  /** @param message Actionable failure text. @returns Nothing; allocates a unique queue identity. */
  private failure(message: string): void {
    this.nextOutcome += 1;
    this.queue.enqueue({
      id: this.nextOutcome,
      kind: 'failure',
      persistent: true,
      message,
    });
  }
  /** Notifies current store subscribers synchronously; subscribers own their cleanup. */
  private notify(): void {
    this.listeners.forEach(
      /** @param listener Store subscriber to invoke. */ (listener) =>
        listener(),
    );
  }
}
