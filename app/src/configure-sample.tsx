import { useCallback, useState } from 'react';
import { ConfigureController } from './configure-controller';
import { ConfigureForm } from './configure-form';
import {
  CONFIGURE_TEXT,
  REPEAT_MODE,
  SPEED_PRESETS,
  type ConfigureSnapshot,
  type ConfigureDraft,
} from './configure-contract';
import { SAMPLE_MACROS } from './sample-macros';

const SAMPLE = SAMPLE_MACROS[0];
export const CONFIGURE_SAMPLE: ConfigureSnapshot = {
  macroId: SAMPLE.id,
  name: SAMPLE.name,
  playback: {
    speed: SPEED_PRESETS[2],
    repeatMode: REPEAT_MODE.once,
    totalRuns: 1,
    intervalMs: 0,
  },
};
const SAMPLE_TEXT = {
  title: 'Controlled Configure example',
  failure: 'Simulate Save failure',
  submitted: 'Draft received; sample library unchanged.',
  cancelled: 'Editor closed. Sample library unchanged.',
  open: 'Open Configure example',
} as const;

/** @returns Controlled editor with switchable async success/failure callbacks; sample library data is never mutated. */
export function ConfigureSample() {
  const [fail, setFail] = useState(false);
  const [log, setLog] = useState('');
  const [controller, setController] = useState<ConfigureController | null>(
    null,
  );
  /** Creates one draft owner with the selected controlled outcome; callbacks receive complete validated properties. */
  function open() {
    setLog('');
    setController(
      new ConfigureController(
        CONFIGURE_SAMPLE,
        /** @param draft Submitted properties. @returns Controlled success/failure after an asynchronous boundary. */ async (
          draft: ConfigureDraft,
        ) => {
          await Promise.resolve();
          setLog(`${SAMPLE_TEXT.submitted} ${JSON.stringify(draft)}`);
          return fail
            ? { ok: false, message: CONFIGURE_TEXT.failure }
            : { ok: true };
        },
      ),
    );
  }
  const closed = useCallback(
    /** Discards the controlled editor and reports closure without changing sample entries. */ () => {
      setController(null);
    },
    [],
  );
  if (controller)
    return <ConfigureForm controller={controller} onClosed={closed} />;
  return (
    <main className="configure-sample">
      <h1>{SAMPLE_TEXT.title}</h1>
      <label>
        <input
          type="checkbox"
          checked={fail}
          onChange={
            /** @param event Controlled failure switch. @returns Nothing. */ (
              event,
            ) => setFail(event.target.checked)
          }
        />
        {SAMPLE_TEXT.failure}
      </label>
      <button type="button" onClick={open}>
        {SAMPLE_TEXT.open}
      </button>
      <p role="status">{log || SAMPLE_TEXT.cancelled}</p>
    </main>
  );
}
