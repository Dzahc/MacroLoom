import { TIME, type Outcome } from './library-model.ts';

export const PAUSE = {
  hover: 'hover',
  focus: 'focus',
  hidden: 'hidden',
} as const;
type PauseReason = (typeof PAUSE)[keyof typeof PAUSE];
type Timer = ReturnType<typeof setTimeout>;
export type ToastClock = {
  now: () => number;
  schedule: (callback: () => void, delay: number) => Timer;
  cancel: (timer: Timer) => void;
};
const DEFAULT_CLOCK: ToastClock = {
  now: () => performance.now(),
  schedule: (callback, delay) => setTimeout(callback, delay),
  cancel: (timer) => clearTimeout(timer),
};

/** Owns notification lifetime independently of React renders. No timer exists until enqueue. */
export class OutcomeQueue {
  private outcomes: readonly Outcome[] = [];
  private listeners = new Set<() => void>();
  private pauses = new Set<PauseReason>();
  private timer: Timer | null = null;
  private remaining: number = TIME.toast;
  private startedAt = 0;

  private clock: ToastClock;
  constructor(clock: ToastClock = DEFAULT_CLOCK) {
    this.clock = clock;
  }

  getSnapshot = (): readonly Outcome[] => this.outcomes;
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  enqueue(outcome: Outcome): void {
    if (this.outcomes.some((item) => item.id === outcome.id)) return;
    this.outcomes = [...this.outcomes, outcome];
    if (this.outcomes.length === 1) this.start();
    this.notify();
  }

  dismiss = (): void => {
    this.cancelTimer();
    this.outcomes = this.outcomes.slice(1);
    this.remaining = TIME.toast;
    this.pauses.delete(PAUSE.hover);
    this.pauses.delete(PAUSE.focus);
    this.start();
    this.notify();
  };

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

  resume(reason: PauseReason): void {
    if (!this.pauses.delete(reason)) return;
    this.start();
  }

  dispose(): void {
    this.cancelTimer();
    this.outcomes = [];
    this.remaining = TIME.toast;
    this.pauses.clear();
  }

  private start(): void {
    if (
      this.outcomes.length === 0 ||
      this.pauses.size !== 0 ||
      this.timer !== null
    )
      return;
    this.startedAt = this.clock.now();
    this.timer = this.clock.schedule(this.dismiss, this.remaining);
  }

  private cancelTimer(): void {
    if (this.timer !== null) this.clock.cancel(this.timer);
    this.timer = null;
  }

  private notify(): void {
    this.listeners.forEach((listener) => listener());
  }
}
