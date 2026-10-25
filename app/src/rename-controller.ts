import {
  configureNameError,
  trimConfigureName,
} from './configure-validation.ts';
import {
  RENAME_POLL_MS,
  RENAME_ATTEMPT_INCREMENT,
  RENAME_TEXT,
  type RenameBinding,
  type RenameCompletion,
  type RenameSnapshot,
  type RenameTransport,
} from './rename-contract.ts';
import { PHASE, type MacroSummary } from './library-model.ts';
import type { SessionSnapshot } from './session-contract.ts';

const INITIAL: RenameSnapshot = {
  draft: null,
  pending: false,
  uncertain: false,
  attempted: false,
  error: '',
  focusId: null,
};

/** Owns one inline draft; native receipts alone determine committed names and uncertain writes are never resubmitted. */
export class RenameController implements RenameBinding {
  private state = INITIAL;
  private listeners = new Set<() => void>();
  private transport: RenameTransport;
  private attemptId: number | null = null;
  private generation = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private startSequence = 0;
  private restoreOnComplete = true;
  private completed: (completion: RenameCompletion) => void;
  /** @param transport Native persistence boundary; construction performs no writes. @param completed Authoritative outcome subscriber. */
  constructor(
    transport: RenameTransport,
    completed: (completion: RenameCompletion) => void = ignoreCompletion,
  ) {
    this.transport = transport;
    this.completed = completed;
  }
  /** @returns Current immutable editor state. */
  getSnapshot = (): RenameSnapshot => this.state;
  /** @returns Current editor state for controlled bindings. */
  get snapshot(): RenameSnapshot {
    return this.state;
  }
  /** @param listener Store observer. @returns Owner cleanup. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Releases this store observer. */
    return () => {
      this.listeners.delete(listener);
    };
  };
  /** @param macro Existing idle row. @returns Nothing; selects its full name in the mounted editor. */
  open = (macro: MacroSummary): void => {
    if (this.state.pending) return;
    this.restoreOnComplete = true;
    this.update({ ...INITIAL, draft: { macroId: macro.id, name: macro.name } });
  };
  /** @param name Untrimmed input. @returns Nothing; validation becomes visible only after Save. */
  change = (name: string): void => {
    if (!this.state.draft || this.state.pending) return;
    const error = this.state.attempted ? configureNameError(name) : '';
    this.update({ ...this.state, draft: { ...this.state.draft, name }, error });
  };
  /** @param restoreFocus Whether keyboard/button dismissal should restore the originating row. @returns Nothing; submitted writes cannot be cancelled. */
  cancel = (restoreFocus = true): void => {
    if (this.state.pending) {
      this.restoreOnComplete = false;
      return;
    }
    const focusId = restoreFocus ? (this.state.draft?.macroId ?? null) : null;
    this.update({ ...INITIAL, focusId });
  };
  /** Validates and submits the current name; backend acknowledgement owns success. @returns Completion with draft retention on failure. */
  save = async (): Promise<void> => {
    if (this.state.uncertain) {
      await this.recover(RENAME_TEXT.uncertain, this.generation);
      return;
    }
    const draft = this.state.draft;
    if (!draft || this.state.pending) return;
    const error = configureNameError(draft.name);
    this.update({ ...this.state, attempted: true, error });
    if (error) return;
    this.update({ ...this.state, pending: true });
    await this.submitDraft(draft);
  };
  /** @param draft Validated name-only draft. @returns Worker submission/reconciliation; stale mounted owners ignore replies. */
  private async submitDraft(
    draft: NonNullable<RenameSnapshot['draft']>,
  ): Promise<void> {
    const generation = this.generation;
    try {
      const status = await this.transport.status();
      if (generation !== this.generation || !this.state.draft) return;
      this.attemptId = status.attemptId + RENAME_ATTEMPT_INCREMENT;
      const completion = await this.transport.submit({
        attemptId: this.attemptId,
        macroId: draft.macroId,
        name: trimConfigureName(draft.name),
      });
      if (generation === this.generation) this.complete(completion);
    } catch (reason) {
      if (generation !== this.generation) return;
      if (this.attemptId !== null) await this.recover(reason, generation);
      else this.failed(reason);
    }
  }
  /** @param session Native lifecycle. @returns Nothing; accepted starts cancel drafts even when their later preflight fails. */
  observe(session: SessionSnapshot): void {
    const started = session.startSequence !== this.startSequence;
    this.startSequence = session.startSequence;
    if ((started || session.phase !== PHASE.idle) && this.state.draft) {
      this.restoreOnComplete = false;
      if (this.attemptId === null) {
        this.generation++;
        this.update(INITIAL);
        return;
      }
      if (this.state.pending && (session.renameSaving || session.deferredStart))
        return;
      this.update({ ...this.state, draft: null, focusId: null });
    }
  }
  /** Cancels owned polling and invalidates pending IPC callbacks on unmount; accepted native writes continue safely. */
  dispose(): void {
    this.generation++;
    clearTimeout(this.timer);
    this.attemptId = null;
    this.state = INITIAL;
  }
  /** @param reason Original IPC diagnostic. @param generation Mounted owner. @returns Receipt reconciliation without another write. */
  private async recover(reason: unknown, generation: number): Promise<void> {
    try {
      const status = await this.transport.status();
      if (generation !== this.generation) return;
      if (status.completion?.attemptId === this.attemptId) {
        this.complete(status.completion);
        return;
      }
      if (status.pending && status.attemptId === this.attemptId) {
        this.poll(reason, generation);
        return;
      }
      this.failed(reason);
    } catch {
      if (generation === this.generation)
        this.update({
          ...this.state,
          pending: true,
          uncertain: true,
          error: RENAME_TEXT.uncertain,
        });
    }
  }
  /** @param reason Retained acknowledgement error. @param generation Current owner. @returns Nothing; schedules one disposable status query. */
  private poll(reason: unknown, generation: number): void {
    this.update({ ...this.state, uncertain: false });
    this.timer = setTimeout(
      /** Reconciles accepted work without resubmission. */ () => {
        void this.recover(reason, generation);
      },
      RENAME_POLL_MS,
    );
  }
  /** @param reason Confirmed precommit rejection. @returns Nothing; preserves the editable draft. */
  private failed(reason: unknown): void {
    this.attemptId = null;
    this.update({
      ...this.state,
      pending: false,
      uncertain: false,
      error: `${RENAME_TEXT.failure}: ${String(reason)}`,
    });
  }
  /** @param completion Authoritative native receipt. @returns Nothing; only a successful or explicitly discarded edit closes. */
  private complete(completion: RenameCompletion): void {
    this.attemptId = null;
    this.completed(completion);
    if (completion.result.ok || completion.discarded || !this.state.draft) {
      this.update({
        ...INITIAL,
        focusId:
          completion.discarded || !this.restoreOnComplete
            ? null
            : (this.state.draft?.macroId ?? null),
      });
      return;
    }
    this.update({
      ...this.state,
      pending: false,
      uncertain: false,
      error: completion.result.message,
    });
  }
  /** @param state Next editor state. @returns Nothing; synchronously notifies current observers. */
  private update(state: RenameSnapshot): void {
    this.state = state;
    this.listeners.forEach(
      /** @param listener Active observer. */ (listener) => listener(),
    );
  }
}

/** @returns Nothing; standalone controller consumers need no outcome callback. */
function ignoreCompletion(): void {}
