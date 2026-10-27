import type { ConfigureCompletion } from './configure-contract.ts';

/** Authoritative saved/failed receipt; the main owner displays it before authorizing dismissal of that exact attempt. */
export type ConfigureRecoveryReceipt = Pick<
  ConfigureCompletion,
  'attemptId' | 'result'
>;

/** Backend-owned display recovery state; the draft remains exclusively in the existing editor. */
export type ConfigureRecoveryStatus = Readonly<{
  occupied: boolean;
  needed: boolean;
  pending: boolean;
  message: string;
  completion: ConfigureRecoveryReceipt | null;
}>;

/** Native commands never submit property saves or accept caller-supplied editor identities. */
export type ConfigureRecoveryTransport = Readonly<{
  status: () => Promise<ConfigureRecoveryStatus>;
  retry: () => Promise<ConfigureRecoveryStatus>;
  discard: (
    observedAttemptId: number | null,
  ) => Promise<ConfigureRecoveryStatus>;
}>;

/** Authoritative recovery state plus transient command/query feedback for main-window controls. */
export type ConfigureRecoveryView = ConfigureRecoveryStatus &
  Readonly<{ working: boolean; error: string }>;

const EMPTY: ConfigureRecoveryView = {
  occupied: false,
  needed: false,
  pending: false,
  message: '',
  working: false,
  error: '',
  completion: null,
};
const NEXT_REVISION = 1;
export const CONFIGURE_PLACEMENT_POLL_MS = 500;

/** Retains recovery availability across transport failures; only explicit actions can retry placement or discard a draft. */
export class ConfigureRecoveryController {
  private readonly transport: ConfigureRecoveryTransport;
  private readonly listeners = new Set<() => void>();
  private state: ConfigureRecoveryView = EMPTY;
  private revision = 0;
  private disposed = false;

  /** @param transport Read-only status and explicit placement/dismissal native commands. */
  constructor(transport: ConfigureRecoveryTransport) {
    this.transport = transport;
  }

  /** @returns Stable presentation snapshot until native state or local command feedback changes. */
  getSnapshot = (): ConfigureRecoveryView => this.state;

  /** @param listener Mounted view subscriber. @returns Idempotent listener cleanup. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Releases this subscriber without changing native editor ownership. */
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** @returns Polling cleanup that invalidates late responses; reconnect supports React's effect remounts. */
  connect(): () => void {
    this.disposed = false;
    if (this.state.working) this.update({ working: false });
    void this.refresh();
    const timer = setInterval(
      /** Queries native recovery state without submitting or moving the editor. */ () => {
        void this.refresh();
      },
      CONFIGURE_PLACEMENT_POLL_MS,
    );
    /** Releases the owned timer and ignores every outstanding native response. */
    return () => {
      clearInterval(timer);
      this.disposed = true;
      this.revision += NEXT_REVISION;
    };
  }

  /** @returns Completion after querying native state; failures retain ownership and block Discard. */
  async refresh(): Promise<void> {
    if (this.disposed || this.state.working) return;
    const revision = (this.revision += NEXT_REVISION);
    try {
      const status = await this.transport.status();
      if (this.current(revision)) this.update({ ...status, error: '' });
    } catch (reason) {
      if (this.current(revision)) this.update({ error: String(reason) });
    }
  }

  /** @returns Completion after explicit draft dismissal; pending or unknown native state never authorizes it. */
  async discard(): Promise<void> {
    if (
      !this.state.needed ||
      this.state.pending ||
      this.state.working ||
      this.state.error ||
      this.disposed
    )
      return;
    await this.perform('discard');
  }

  /** @returns Completion after explicitly retrying placement of the existing editor; never submits a Save. */
  async retry(): Promise<void> {
    if (!this.state.needed || this.state.working || this.disposed) return;
    await this.perform('retry');
  }

  /** @param command Explicit placement/dismissal action. @returns Native result delivery, ignoring queries/results superseded by this action or teardown. */
  private async perform(command: 'retry' | 'discard'): Promise<void> {
    const revision = (this.revision += NEXT_REVISION);
    this.update({ working: true });
    try {
      const status = await (command === 'discard'
        ? this.transport.discard(this.state.completion?.attemptId ?? null)
        : this.transport.retry());
      if (this.current(revision)) this.update({ ...status, error: '' });
    } catch (reason) {
      if (this.current(revision)) this.update({ error: String(reason) });
    } finally {
      if (this.current(revision)) this.update({ working: false });
    }
  }

  /** @param revision Request generation. @returns Whether this response still belongs to the mounted view's latest operation. */
  private current(revision: number): boolean {
    return !this.disposed && revision === this.revision;
  }

  /** @param patch Native state or local feedback. @returns Nothing; synchronously notifies mounted subscribers. */
  private update(patch: Partial<ConfigureRecoveryView>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach(
      /** @param listener Mounted subscriber. @returns Nothing. */ (listener) =>
        listener(),
    );
  }
}
