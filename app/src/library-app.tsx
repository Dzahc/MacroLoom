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

const DevelopmentPane = import.meta.env.DEV
  ? lazy(() =>
      import('./development-pane').then((module) => ({
        default: module.DevelopmentPane,
      })),
    )
  : null;

export function LibraryApp() {
  const [scenario, setScenario] = useState<ScenarioName>(SCENARIO.populated);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showDev, setShowDev] = useState(false);
  const [log, setLog] = useState<readonly string[]>([]);
  const [queue] = useState(() => new OutcomeQueue());
  const nextOutcome = useRef(0);
  const sample = SCENARIOS[scenario];
  const snapshot = {
    ...sample,
    selectedId: reconcileSelection(sample.macros, selectedId),
  };
  useEffect(() => () => queue.dispose(), [queue]);

  function appendLog(message: string) {
    setLog((current) => [...current, message].slice(-DEV_TEXT.maxLog));
  }
  function changeScenario(next: ScenarioName) {
    const nextSample = SCENARIOS[next];
    setSelectedId((current) => reconcileSelection(nextSample.macros, current));
    setScenario(next);
    nextSample.failures.forEach((failure) => {
      console.warn(failure);
      appendLog(failure);
    });
  }
  function select(change: SelectionChange) {
    setSelectedId(change.macroId);
    appendLog(JSON.stringify(change));
  }
  function action(request: LibraryAction) {
    appendLog(JSON.stringify(request));
  }
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
              onClick={() => setShowDev((current) => !current)}
            >
              {DEV_TEXT.open}
            </button>
          </div>
        )}
        <LibraryView
          snapshot={snapshot}
          onSelect={select}
          onAction={action}
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
            onClear={() => setLog([])}
            onClose={() => setShowDev(false)}
          />
        </Suspense>
      )}
    </div>
  );
}
