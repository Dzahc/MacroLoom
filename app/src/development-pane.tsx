import {
  DEV_TEXT,
  OUTCOME_EXAMPLES,
  SCENARIOS,
  type ScenarioName,
} from './library-samples';
import type { Outcome } from './library-model';

type Example = Omit<Outcome, 'id'>;
type Props = {
  scenario: ScenarioName;
  onScenario: (scenario: ScenarioName) => void;
  onOutcome: (outcome: Example) => void;
  log: readonly string[];
  onClear: () => void;
  onClose: () => void;
};

export function DevelopmentPane(props: Props) {
  return (
    <aside className="development-pane" aria-label={DEV_TEXT.title}>
      <header>
        <h2>{DEV_TEXT.title}</h2>
        <button type="button" onClick={props.onClose}>
          {DEV_TEXT.close}
        </button>
      </header>
      <p>{DEV_TEXT.hint}</p>
      <fieldset>
        <legend>{DEV_TEXT.scenarios}</legend>
        <div className="scenario-buttons">
          {(Object.keys(SCENARIOS) as ScenarioName[]).map((key) => (
            <button
              type="button"
              key={key}
              aria-pressed={props.scenario === key}
              onClick={() => props.onScenario(key)}
            >
              {SCENARIOS[key].label}
            </button>
          ))}
        </div>
      </fieldset>
      <fieldset>
        <legend>{DEV_TEXT.toasts}</legend>
        <div className="scenario-buttons">
          {OUTCOME_EXAMPLES.map((example) => (
            <button
              type="button"
              key={example.label}
              onClick={() => props.onOutcome(example)}
            >
              {example.label}
            </button>
          ))}
        </div>
      </fieldset>
      <div className="log-heading">
        <h3>{DEV_TEXT.log}</h3>
        <button type="button" onClick={props.onClear}>
          {DEV_TEXT.clear}
        </button>
      </div>
      <ol className="action-log">
        {props.log.map((entry, index) => (
          <li key={index}>{entry}</li>
        ))}
      </ol>
      {props.log.length === 0 && <p>{DEV_TEXT.emptyLog}</p>}
    </aside>
  );
}
