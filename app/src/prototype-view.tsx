import type { ReactNode } from 'react';

export type Snapshot = {
  phase: 'idle' | 'recording' | 'playing' | 'stopping';
  eventCount: number;
  dragMoveCount: number;
  lastRelease: [number, number] | null;
  lastReplayRelease: [number, number] | null;
  durationMs: number;
  message: string;
  captureGapMinMs: number | null;
  dispatchLatenessP95Ms: number | null;
  dispatchLatenessMaxMs: number | null;
  stopResponseMs: number | null;
  f9Available: boolean;
  f8Available: boolean;
};

export type Command = 'start_recording' | 'stop' | 'play';
type StateProps = { state: Snapshot | null };
type ActionProps = { onAction: (command: Command) => Promise<void> };

const MILLISECONDS_PER_SECOND = 1000;
const DISPLAY_PRECISION = 1;
const MISSING_METRIC = '—';
/** @param value Millisecond duration or missing initial state. @returns Seconds with one decimal place. */
function durationSeconds(value: number | undefined): string {
  return ((value ?? 0) / MILLISECONDS_PER_SECOND).toFixed(DISPLAY_PRECISION);
}

/** @param value Available timing/count metric. @returns The value or an explicit missing indicator. */
function displayMetric(value: number | null | undefined): number | string {
  return value ?? MISSING_METRIC;
}

/** @param value Recorded/replayed physical point. @returns Signed coordinate text or an explicit missing indicator. */
function displayPoint(value: [number, number] | null | undefined): string {
  return value?.join(', ') ?? MISSING_METRIC;
}

/** @param props Backend state and native action callback. @returns Guarded temporary capture/replay controls. */
function Controls({ state, onAction }: StateProps & ActionProps) {
  const active = (state?.phase ?? 'idle') !== 'idle';
  return (
    <nav aria-label="Prototype controls">
      <button
        onClick={() => void onAction('start_recording')}
        disabled={active || !state?.f8Available}
      >
        ● <span>Record</span>
        <kbd>F9</kbd>
      </button>
      <button
        onClick={() => void onAction('play')}
        disabled={active || !state?.eventCount || !state?.f8Available}
      >
        ▶ <span>Play</span>
      </button>
      <button onClick={() => void onAction('stop')} disabled={!active}>
        ■ <span>Stop</span>
        <kbd>F8</kbd>
      </button>
    </nav>
  );
}

/** @param props Current lifecycle and command error. @returns Readable status through active cleanup. */
function StatusBanner({
  state,
  error,
  phase,
}: StateProps & { error: string; phase: Snapshot['phase'] }) {
  const labels = {
    idle: 'Ready',
    recording: 'Recording',
    playing: 'Playing',
    stopping: 'Stopping…',
  };
  return (
    <section className={`banner ${phase}`} role="status" aria-live="polite">
      <span className="dot" />
      <div>
        <strong>{labels[phase]}</strong>
        <p>{error || state?.message || 'Starting…'}</p>
      </div>
      {phase !== 'idle' && <time>{durationSeconds(state?.durationMs)}s</time>}
    </section>
  );
}

/** @param props Metric label and available value. @returns A bounded development metric. */
function Metric({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

/** @param props Native release points. @returns Recorded/replayed coordinates when available. */
function ReleaseMetric({ state }: StateProps) {
  return (
    <Metric
      label="Release pixel: record / replay"
      value={
        <>
          {displayPoint(state?.lastRelease)} /{' '}
          {displayPoint(state?.lastReplayRelease)}
        </>
      }
    />
  );
}

/** @param props Available native metrics and measured command-to-frame response. @returns Development timing feedback. */
function TimingMetrics({
  state,
  uiResponse,
}: StateProps & { uiResponse: number | null }) {
  return (
    <>
      <Metric
        label="Capture minimum gap"
        value={<>{displayMetric(state?.captureGapMinMs)} ms</>}
      />
      <Metric
        label="Dispatch lateness p95 / max"
        value={
          <>
            {displayMetric(state?.dispatchLatenessP95Ms)} /{' '}
            {displayMetric(state?.dispatchLatenessMaxMs)} ms
          </>
        }
      />
      <Metric
        label="Stop response"
        value={<>{displayMetric(state?.stopResponseMs)} ms</>}
      />
      <Metric
        label="Last UI response"
        value={<>{displayMetric(uiResponse)} ms</>}
      />
    </>
  );
}

/** @param props Temporary recording and available metrics. @returns Unsaved capture details and shared-input instructions. */
function IdleContent({
  state,
  uiResponse,
}: StateProps & { uiResponse: number | null }) {
  return (
    <section className="content">
      <h2>In-memory take</h2>
      <p className="count">
        {state?.eventCount ?? 0} events · {durationSeconds(state?.durationMs)}s
      </p>
      <p>
        Focus another ordinary app, press F9, then click its input field and
        type or drag. Press F8 to stop. Return the target to the same screen
        position before Play.
      </p>
      <div className="metrics">
        <Metric
          label="Drag moves"
          value={displayMetric(state?.dragMoveCount)}
        />
        <ReleaseMetric state={state} />
        <TimingMetrics state={state} uiResponse={uiResponse} />
      </div>
      <p className="footnote">
        Prototype data disappears when this app closes. Physical input uses the
        shared Windows cursor and keyboard focus.
      </p>
    </section>
  );
}

/** @param props Authoritative prototype presentation and native controls. @returns Optional development-only temporary session view. */
export function PrototypeView({
  state,
  error,
  uiResponse,
  onAction,
}: StateProps & ActionProps & { error: string; uiResponse: number | null }) {
  const phase = state?.phase ?? 'idle';
  const active = phase !== 'idle';
  return (
    <main className={active ? 'active' : ''}>
      <header>
        <div className="brand">
          <span className="logo">M</span>
          <div>
            <strong>MacroLoom</strong>
            <small>Windows input prototype</small>
          </div>
        </div>
        <span className="prototype">ML-8</span>
      </header>
      <Controls state={state} onAction={onAction} />
      <StatusBanner state={state} error={error} phase={phase} />
      {!active && <IdleContent state={state} uiResponse={uiResponse} />}
    </main>
  );
}
