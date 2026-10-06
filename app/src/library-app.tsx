import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { LibraryView } from './library-view';
import {
  reconcileSelection,
  type LibraryAction,
  type Outcome,
  type SelectionChange,
} from './library-model';
import {
  DEV_TEXT,
  SCENARIO,
  SCENARIOS,
  type ScenarioName,
} from './library-samples';
import { OutcomeQueue } from './outcome-queue';
import { OutcomeToasts } from './outcome-toasts';
import { isTauri } from '@tauri-apps/api/core';
import { LiveLibraryApp } from './live-library-app';

const DELETE_LOG = {
  confirmed: 'delete-confirmed',
  cancelled: 'delete-cancelled',
} as const;

const DevelopmentPane = import.meta.env.DEV
  ? lazy(
      /** @returns The development module promise; chunk loading failures reach Suspense's host boundary. */
      () =>
        import('./development-pane').then(
          /** @param module Loaded pane module. @returns React's lazy default-export shape. */
          (module) => ({
            default: module.DevelopmentPane,
          }),
        ),
    )
  : null;

/**
 * Owns controlled sample state, callback logging, and the outcome queue.
 * @returns The library plus an optional development pane; no live input or disk effects occur.
 * Queue timers are disposed on unmount and the log retains only its bounded tail.
 */
function SampleLibraryApp() {
  const [scenario, setScenario] = useState<ScenarioName>(SCENARIO.populated);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showDev, setShowDev] = useState(false);
  const [log, setLog] = useState<readonly string[]>([]);
  const [queue] = useState(
    /** @returns One owner-managed queue for this mounted application. */
    () => new OutcomeQueue(),
  );
  const nextOutcome = useRef(0);
  const sample = SCENARIOS[scenario];
  const snapshot = {
    ...sample,
    selectedId: reconcileSelection(sample.macros, selectedId),
  };
  useEffect(
    /** @returns Unmount cleanup that disposes the owned outcome queue. */
    () =>
      /** Cancels queued timers and clears outcomes on unmount. */
      () =>
        queue.dispose(),
    [queue],
  );

  /** @param message Entry appended to the bounded development log. @returns Nothing. */
  function appendLog(message: string) {
    setLog(
      /** @param current Previous log. @returns Its bounded tail including the new message. */
      (current) => [...current, message].slice(-DEV_TEXT.maxLog),
    );
  }
  /**
   * Applies a sample scenario, reconciles selection, and logs omitted-file failures.
   * @param next Known sample scenario key.
   * @returns Nothing; failures are written to console and the development log only.
   */
  function changeScenario(next: ScenarioName) {
    const nextSample = SCENARIOS[next];
    setSelectedId(
      /** @param current Previous identity. @returns Selection reconciled against the next scenario. */
      (current) => reconcileSelection(nextSample.macros, current),
    );
    setScenario(next);
    nextSample.failures.forEach(
      /** @param failure Omitted-file diagnostic written to console and dev log. @returns Nothing. */
      (failure) => {
        console.warn(failure);
        appendLog(failure);
      },
    );
  }
  /** @param change Clicked identity/source to select and log. @returns Nothing. */
  function select(change: SelectionChange) {
    setSelectedId(change.macroId);
    appendLog(JSON.stringify(change));
  }
  /** @param request Typed action recorded in the log without executing it. @returns Nothing. */
  function action(request: LibraryAction) {
    appendLog(JSON.stringify(request));
  }
  /** @param macroId Deliberately confirmed stable ID, logged while sample data remains intact. @returns Nothing. */
  function confirmDelete(macroId: string) {
    appendLog(JSON.stringify({ event: DELETE_LOG.confirmed, macroId }));
  }
  /** @param macroId Cancelled dialog target, logged without emitting an action. @returns Nothing. */
  function cancelDelete(macroId: string) {
    appendLog(JSON.stringify({ event: DELETE_LOG.cancelled, macroId }));
  }
  /** @param outcome Sample message/kind to enqueue with a unique local ID. @returns Nothing. */
  function notify(outcome: Omit<Outcome, 'id'>) {
    nextOutcome.current += 1;
    queue.enqueue({ ...outcome, id: nextOutcome.current });
  }
  return (
    <div className={`app-layout ${showDev ? 'with-development' : ''}`}>
      <div className="product-column">
        {import.meta.env.DEV && (
          <div className="development-toggle">
            <button
              type="button"
              onClick={
                /** Toggles the host-owned development pane. */
                () =>
                  setShowDev(
                    /** @param current Previous pane visibility. @returns Its inverse. */
                    (current) => !current,
                  )
              }
            >
              {DEV_TEXT.open}
            </button>
          </div>
        )}
        <LibraryView
          snapshot={snapshot}
          onSelect={select}
          onAction={action}
          onDeleteConfirmed={confirmDelete}
          onDeleteCancelled={cancelDelete}
          notifications={<OutcomeToasts queue={queue} />}
        />
      </div>
      {DevelopmentPane && showDev && (
        <Suspense>
          <DevelopmentPane
            scenario={scenario}
            onScenario={changeScenario}
            onOutcome={notify}
            log={log}
            onClear={/** Clears the local development log. */ () => setLog([])}
            onClose={
              /** Hides the development pane without changing library state. */ () =>
                setShowDev(false)
            }
          />
        </Suspense>
      )}
    </div>
  );
}

/** @returns Native library with persisted deletion, or nonmutating controlled browser samples. */
export function LibraryApp() {
  return isTauri() ? <LiveLibraryApp /> : <SampleLibraryApp />;
}
