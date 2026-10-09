import {
  CONFIGURE_FIELD,
  CONFIGURE_TEXT,
  type ConfigureErrors,
  type ConfigureField,
  type ConfigureSave,
  type ConfigureSnapshot,
} from './configure-contract.ts';
import {
  configureDraft,
  configureErrors,
  configureInputs,
  type ConfigureInputs,
} from './configure-validation.ts';

const INITIAL_FOCUS_REVISION = 0;
const FOCUS_REVISION_INCREMENT = 1;
const EMPTY_MESSAGE = '';

export type ConfigureState = Readonly<{
  input: ConfigureInputs;
  errors: ConfigureErrors;
  pending: boolean;
  closed: boolean;
  message: string;
  focus: ConfigureField | null;
  focusRevision: number;
}>;

/** Owns local property drafts and serializes asynchronous Save attempts. Cancellation never submits and pending attempts cannot be dismissed. */
export class ConfigureController {
  private readonly original: ConfigureSnapshot;
  private readonly save: ConfigureSave;
  private readonly listeners = new Set<() => void>();
  private touched = new Set<ConfigureField>();
  private state: ConfigureState;

  /** @param snapshot Trusted baseline copied into local strings. @param save Async consumer; rejection becomes a retained form error. */
  constructor(snapshot: ConfigureSnapshot, save: ConfigureSave) {
    this.original = { ...snapshot, playback: { ...snapshot.playback } };
    this.save = save;
    this.state = {
      input: configureInputs(snapshot),
      errors: {},
      pending: false,
      closed: false,
      message: EMPTY_MESSAGE,
      focus: null,
      focusRevision: INITIAL_FOCUS_REVISION,
    };
  }
  /** @returns Stable immutable presentation state between edits. */
  getSnapshot = (): ConfigureState => this.state;
  /** @param listener View subscriber. @returns Idempotent listener cleanup. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Releases only this view subscriber; the asynchronous consumer owns its work. */
    return () => {
      this.listeners.delete(listener);
    };
  };
  /** @param field Edited input key. @param value Typed local value. @returns Nothing; pending/closed editors ignore edits. */
  edit<K extends ConfigureField>(field: K, value: ConfigureInputs[K]): void {
    if (this.state.pending || this.state.closed) return;
    const input = { ...this.state.input, [field]: value };
    const activeErrors = configureErrors(input);
    const errors: ConfigureErrors = {};
    for (const key of this.touched)
      if (activeErrors[key]) errors[key] = activeErrors[key];
    this.update({ input, errors, message: EMPTY_MESSAGE });
  }
  /** @param field Input losing focus. @returns Nothing; exposes its current active validation error. */
  blur(field: ConfigureField): void {
    if (this.state.pending || this.state.closed) return;
    this.touched.add(field);
    this.update({
      errors: {
        ...this.state.errors,
        [field]: configureErrors(this.state.input)[field],
      },
    });
  }
  /** @returns Whether cancellation closed the idle editor; never calls Save. */
  cancel(): boolean {
    if (this.state.pending || this.state.closed) return false;
    this.update({ closed: true });
    return true;
  }
  /** @returns Completion after one callback or validation; repeat/reentrant activation cannot submit twice and failures retain all strings. */
  async submit(): Promise<void> {
    if (this.state.pending || this.state.closed) return;
    this.touched = new Set(CONFIGURE_FIELD);
    const errors = configureErrors(this.state.input);
    const focus =
      CONFIGURE_FIELD.find(
        /** @param field Ordered input. @returns Whether it is invalid. */ (
          field,
        ) => errors[field],
      ) ?? null;
    this.update({
      errors,
      focus,
      focusRevision: this.state.focusRevision + FOCUS_REVISION_INCREMENT,
      message: EMPTY_MESSAGE,
    });
    if (focus) return;
    this.update({ pending: true });
    try {
      const result = await this.save(
        configureDraft(this.state.input, this.original),
      );
      if (result.ok) this.update({ pending: false, closed: true });
      else this.failed(result.message, result.fields ?? {});
    } catch {
      this.failed(CONFIGURE_TEXT.failure, {});
    }
  }
  /** @param message Actionable consumer error. @param errors Optional field errors. @returns Nothing; resets the submission guard for deliberate Retry. */
  private failed(message: string, errors: ConfigureErrors): void {
    const focus =
      CONFIGURE_FIELD.find(
        /** @param field Input key. @returns Whether the consumer rejected it. */ (
          field,
        ) => errors[field],
      ) ?? null;
    this.update({
      pending: false,
      message,
      errors,
      focus,
      focusRevision: this.state.focusRevision + FOCUS_REVISION_INCREMENT,
    });
  }
  /** @param patch State changes. @returns Nothing; synchronously notifies the mounted views. */
  private update(patch: Partial<ConfigureState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach(
      /** @param listener Subscriber. @returns Nothing. */ (listener) =>
        listener(),
    );
  }
}
