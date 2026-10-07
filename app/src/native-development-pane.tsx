import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  COMPACT,
  WINDOW_COMMAND,
  type SessionPresentation,
} from './compact-contract';
import { isCompactPhase } from './compact-presentation';
import {
  SESSION_SAMPLES,
  SESSION_SCENARIO,
  advanceSample,
  previewStatus,
  type SessionScenario,
} from './session-samples';

const PREVIEW_TEXT = {
  title: 'Compact development preview',
  hint: 'Switch presentation states directly. No recording, playback, shortcuts, or file changes are started.',
  delay: 'Apply after 3 seconds (time to focus another application)',
  progress: 'Publish progress at 10 updates per second',
  waiting: 'Waiting to apply the selected view…',
  applied: 'Presentation applied to the main window.',
  failed: 'Could not apply presentation',
  states: 'Main window presentation',
} as const;

/** @returns Development-only native controls in their own window, preserving access while the main view is compact. */
export function NativeDevelopmentPane() {
  const [scenario, setScenario] = useState<SessionScenario>(
    SESSION_SCENARIO.idle,
  );
  const [delayed, setDelayed] = useState(false);
  const [animate, setAnimate] = useState(true);
  const [message, setMessage] = useState('');
  useEffect(
    /** Publishes direct controlled state; delayed application and progress timers are cancelled on scenario changes/unmount. */
    () => {
      let disposed = false;
      let pending = false;
      let interval: ReturnType<typeof setInterval> | undefined;
      const seed = SESSION_SAMPLES[scenario].status;
      /** @param status Monotonic demonstration progress. @returns Nothing; throttles outstanding IPC and reports failures in this development window. */
      function publish(status: SessionPresentation) {
        if (disposed || pending) return;
        pending = true;
        void invoke<SessionPresentation>(WINDOW_COMMAND.preview, {
          status: previewStatus(status),
        })
          .then(
            /** Records successful application without changing main-window focus. */
            () => {
              if (!disposed) setMessage(PREVIEW_TEXT.applied);
            },
          )
          .catch(
            /** @param reason Native precondition/validation failure. @returns Nothing. */
            (reason: unknown) => {
              if (!disposed)
                setMessage(`${PREVIEW_TEXT.failed}: ${String(reason)}`);
            },
          )
          .finally(
            /** Releases IPC throttling regardless of command success. */
            () => {
              pending = false;
            },
          );
      }
      const timer = setTimeout(
        /** Starts the selected view directly; progress never advances to another phase automatically. */
        () => {
          const started = performance.now();
          publish(seed);
          if (animate && isCompactPhase(seed.phase))
            interval = setInterval(
              /** Publishes controlled elapsed/countdown data without performing session work. */
              () => publish(advanceSample(seed, performance.now() - started)),
              COMPACT.progressInterval,
            );
        },
        delayed ? COMPACT.applyDelay : 0,
      );
      /** Cancels pending view application and all demonstration progress timers. */
      return () => {
        disposed = true;
        clearTimeout(timer);
        if (interval) clearInterval(interval);
      };
    },
    [scenario, delayed, animate],
  );
  return (
    <aside
      className="development-pane native-development"
      aria-label={PREVIEW_TEXT.title}
    >
      <h2>{PREVIEW_TEXT.title}</h2>
      <p>{PREVIEW_TEXT.hint}</p>
      <label>
        <input
          type="checkbox"
          checked={delayed}
          onChange={
            /** @param event Delay preference. @returns Nothing; affects the next controlled application. */
            (event) => setDelayed(event.target.checked)
          }
        />
        {PREVIEW_TEXT.delay}
      </label>
      <label>
        <input
          type="checkbox"
          checked={animate}
          onChange={
            /** @param event Progress preference. @returns Nothing; tears down/reconnects controlled timers. */
            (event) => setAnimate(event.target.checked)
          }
        />
        {PREVIEW_TEXT.progress}
      </label>
      <fieldset>
        <legend>{PREVIEW_TEXT.states}</legend>
        <div className="scenario-buttons">
          {(Object.keys(SESSION_SAMPLES) as SessionScenario[]).map(
            /** @param key Scenario identity. @returns A direct presentation-state switch, independent of toolbar actions. */
            (key) => (
              <button
                type="button"
                key={key}
                aria-pressed={scenario === key}
                onClick={
                  /** Selects this presentation without implementing Record, Play, or Stop. */
                  () => {
                    setScenario(key);
                    setMessage(delayed ? PREVIEW_TEXT.waiting : '');
                  }
                }
              >
                {SESSION_SAMPLES[key].label}
              </button>
            ),
          )}
        </div>
      </fieldset>
      <p role="status">{message}</p>
    </aside>
  );
}
