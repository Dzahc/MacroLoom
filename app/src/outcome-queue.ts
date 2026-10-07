import { TIME, type Outcome } from './library-model.ts';

export const PAUSE = {
  hover: 'hover',
  focus: 'focus',
  hidden: 'hidden',
  compact: 'compact',
} as const;
type PauseReason = (typeof PAUSE)[keyof typeof PAUSE];
type Timer = ReturnType<typeof setTimeout>;
export type ToastClock = {
  now: () => number;
  schedule: (callback: () => void, delay: number) => Timer;
  cancel: (timer: Timer) => void;
};
const DEFAULT_CLOCK: ToastClock = {
  /** Returns a monotonic timestamp in milliseconds. */
  now: () => performance.now(),
  /** Schedules callback after delay milliseconds and returns its cancellable handle. */
  schedule: (callback, delay) => setTimeout(callback, delay),
  /** Cancels the supplied timer handle without running its callback. */
  cancel: (timer) => clearTimeout(timer),
};

/**
 * Serializes outcomes independently of React renders. Only the first outcome has
 * a timer; every pause reason must clear before its remaining lifetime resumes.
 * The owner must dispose the queue on unmount to release timers.
 */
export class OutcomeQueue {
  private outcomes: readonly Outcome[] = [];
  private listeners = new Set<() => void>();
  private pauses = new Set<PauseReason>();
  private timer: Timer | null = null;
  private remaining: number = TIME.toast;
  private startedAt = 0;

  private clock: ToastClock;
  /**
   * Creates an empty queue with no scheduled work.
   * @param clock Monotonic millisecond clock and cancellable scheduler; defaults to the browser clock.
   */
  constructor(clock: ToastClock = DEFAULT_CLOCK) {
    this.clock = clock;
  }

  /** @returns The stable immutable outcome array until enqueue or dismissal changes it. */
  getSnapshot = (): readonly Outcome[] => this.outcomes;
  /**
   * Registers a React store listener without notifying immediately.
   * @param listener Called synchronously when the outcome array changes.
   * @returns Idempotent cleanup that removes this listener.
   */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Releases the captured listener without changing outcomes or timer state. */
    return () => {
      this.listeners.delete(listener);
    };
  };

  /**
   * Appends an immutable outcome; duplicate IDs already in the queue are ignored.
   * @param outcome Notification with a unique identity and user-facing message.
   * @returns Nothing; starts the first unpaused timer and notifies subscribers.
   */
  enqueue(outcome: Outcome): void {
    if (
      this.outcomes.some(
        /** @param item Queued outcome. @returns Whether its identity duplicates the incoming outcome. */
        (item) => item.id === outcome.id,
      )
    )
      return;
    this.outcomes = [...this.outcomes, outcome];
    if (this.outcomes.length === 1) this.start();
    this.notify();
  }

  /**
   * Removes the current outcome and gives its successor a full lifetime.
   * Clears element-local hover/focus pauses; document pauses persist.
   * @returns Nothing; cancels the old timer and synchronously notifies subscribers.
   */
  dismiss = (): void => {
    this.cancelTimer();
    this.outcomes = this.outcomes.slice(1);
    this.remaining = TIME.toast;
    this.pauses.delete(PAUSE.hover);
    this.pauses.delete(PAUSE.focus);
    this.start();
    this.notify();
  };

  /**
   * Preserves remaining visible time and suspends expiration idempotently.
   * @param reason Independent cause of suspension; repeating it has no effect.
   * @returns Nothing; cancels any running timer without changing outcomes.
   */
  pause(reason: PauseReason): void {
    if (this.pauses.has(reason)) return;
    if (this.timer !== null) {
      this.remaining = Math.max(
        0,
        this.remaining - (this.clock.now() - this.startedAt),
      );
      this.cancelTimer();
    }
    this.pauses.add(reason);
  }

  /**
   * Clears one suspension reason and resumes only when all reasons are cleared.
   * @param reason Previously added suspension reason; absent reasons are ignored.
   * @returns Nothing; may schedule the current outcome's remaining lifetime.
   */
  resume(reason: PauseReason): void {
    if (!this.pauses.delete(reason)) return;
    this.start();
  }

  /**
   * Cancels scheduled work, empties the queue, and resets lifetime and pauses.
   * Subscribers own their unsubscribe callbacks; disposal does not notify them.
   * @returns Nothing; safe to repeat during owner cleanup.
   */
  dispose(): void {
    this.cancelTimer();
    this.outcomes = [];
    this.remaining = TIME.toast;
    this.pauses.clear();
  }

  /** Schedules remaining milliseconds only for a nonempty, unpaused queue with no timer. */
  private start(): void {
    if (
      this.outcomes.length === 0 ||
      this.pauses.size !== 0 ||
      this.timer !== null ||
      this.outcomes[0]?.persistent === true
    )
      return;
    this.startedAt = this.clock.now();
    this.timer = this.clock.schedule(this.dismiss, this.remaining);
  }

  /** Cancels the current handle, if present, and restores the no-timer invariant. */
  private cancelTimer(): void {
    if (this.timer !== null) this.clock.cancel(this.timer);
    this.timer = null;
  }

  /** Synchronously invokes subscribers; subscriber failures propagate to the caller. */
  private notify(): void {
    this.listeners.forEach(
      /** @param listener Subscriber to notify synchronously; failures propagate. @returns Nothing. */
      (listener) => listener(),
    );
  }
}
