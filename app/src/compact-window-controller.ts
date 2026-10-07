import type {
  WindowViewRequest,
  WindowViewResult,
} from './compact-contract.ts';
import { OutcomeQueue } from './outcome-queue.ts';

export type WindowTransport = (
  request: WindowViewRequest,
) => Promise<WindowViewResult>;
type ViewState = Readonly<{ compact: boolean | null; failed: boolean }>;
const INITIAL_VIEW: ViewState = { compact: null, failed: false };

/** Serializes native view changes and reports actual layout without implementing session operations. */
export class CompactWindowController {
  readonly failures = new OutcomeQueue();
  private state: ViewState = INITIAL_VIEW;
  private listeners = new Set<() => void>();
  private tail: Promise<void> = Promise.resolve();
  private transport: WindowTransport;
  private lastRequest = '';
  private nextOutcome = 0;
  private generation = 0;
  private disposed = false;
  private blockedMode: boolean | null = null;
  private nativeError: string | null = null;

  /** @param transport Typed native boundary; transition failures become timed toasts. */
  constructor(transport: WindowTransport) {
    this.transport = transport;
  }

  /** @returns Stable actual-mode snapshot; null uses the requested layout before the first result. */
  getSnapshot = (): ViewState => this.state;

  /** @param listener View-store subscriber. @returns Idempotent unsubscribe. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    /** Detaches this listener without changing native mode. */
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Reconnects a StrictMode owner without discarding the native saved full-window placement. */
  activate(): void {
    this.disposed = false;
    this.lastRequest = '';
    this.blockedMode = null;
  }

  /** @param request Measured content and desired mode. @returns Completion after earlier native transitions; duplicate measurements are ignored. */
  update = (request: WindowViewRequest): Promise<void> => {
    const key = JSON.stringify(request);
    if (request.compact === this.blockedMode) return this.tail;
    if (this.lastRequest === key) return this.tail;
    this.lastRequest = key;
    const generation = this.generation;
    this.tail = this.tail.then(
      /** Applies this request only while its owner generation remains mounted. */
      async () => {
        if (this.disposed || generation !== this.generation) return;
        if (request.compact === this.blockedMode) return;
        this.blockedMode = null;
        const result = await this.transition(request);
        if (this.disposed || generation !== this.generation) return;
        this.state = { compact: result.compact, failed: result.error !== null };
        if (result.error !== null) {
          this.blockedMode = request.compact;
          this.report(result.error);
        }
        this.notify();
      },
    );
    return this.tail;
  };

  /** Releases timers and invalidates pending callbacks; native ownership ends with the main window. */
  dispose(): void {
    this.disposed = true;
    this.generation += 1;
    this.failures.dispose();
  }

  /** @param message Transition/development-window failure. @returns Nothing; queues a transient notification with its own unique identity. */
  report(message: string): void {
    if (this.disposed) return;
    this.nextOutcome += 1;
    this.failures.enqueue({ id: this.nextOutcome, kind: 'failure', message });
  }

  /** @param result Actual mode after a native movement/DPI correction. @returns Nothing; ordinary progress never invokes this path. */
  receive(result: WindowViewResult): void {
    if (this.disposed) return;
    if (result.error !== null && result.error !== this.nativeError)
      this.report(result.error);
    this.nativeError = result.error;
    this.state = { compact: result.compact, failed: result.error !== null };
    this.notify();
  }

  /** @param request Requested native view. @returns Actual mode, with rejected commands converted to timed failure results. */
  private async transition(
    request: WindowViewRequest,
  ): Promise<WindowViewResult> {
    try {
      return await this.transport(request);
    } catch (reason: unknown) {
      return { compact: this.state.compact ?? false, error: String(reason) };
    }
  }

  /** Notifies mounted view subscribers synchronously without manipulating focus. */
  private notify(): void {
    this.listeners.forEach(
      /** @param listener Store subscriber. @returns Nothing. */
      (listener) => listener(),
    );
  }
}
