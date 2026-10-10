import {
  useEffect,
  useLayoutEffect,
  useRef,
  useSyncExternalStore,
  type KeyboardEvent,
} from 'react';
import {
  ConfigureController,
  type ConfigureState,
} from './configure-controller';
import {
  CONFIGURE_TEXT,
  CONFIGURE_FIELD_KEY,
  CONFIGURE_INPUT_ID,
  REPEAT_MODE,
  SPEED_PRESETS,
  type ConfigureField,
} from './configure-contract';

const KEY = {
  enter: 'Enter',
  escape: 'Escape',
  tab: 'Tab',
  space: ' ',
} as const;
const ENABLED_CONTROLS =
  'input:not(:disabled), select:not(:disabled), button:not(:disabled)';
const FIRST = 0;
const LAST_OFFSET = 1;
const PROGRAMMATIC_TAB_INDEX = -1;
const EMPTY_STATUS = '';
const SPEED_MULTIPLIER_SUFFIX = '×';
const TITLE_ID = 'configure-title';
const ERROR_SUFFIX = '-error';
const HINT_SUFFIX = '-hint';
type FieldProps = {
  field: ConfigureField;
  label: string;
  state: ConfigureState;
  children: React.ReactNode;
  hint?: string;
};

/** @param field Stable field key. @returns Document-local input identifier. */
function inputId(field: ConfigureField): string {
  return CONFIGURE_INPUT_ID[field];
}

/** @param props Field label, error, hint and input. @returns Explicitly associated accessible label and field-specific diagnostics. */
function Field({ field, label, state, children, hint }: FieldProps) {
  const id = inputId(field);
  return (
    <div className="configure-field">
      <label htmlFor={id}>{label}</label>
      {children}
      {hint && (
        <p className="configure-hint" id={`${id}${HINT_SUFFIX}`}>
          {hint}
        </p>
      )}
      {state.errors[field] && (
        <p className="configure-error" id={`${id}${ERROR_SUFFIX}`} role="alert">
          {state.errors[field]}
        </p>
      )}
    </div>
  );
}

/** @param field Input key. @param state Current errors. @param hint Whether a hint is present. @returns ARIA associations and invalid state for the actual control. */
function accessibility(
  field: ConfigureField,
  state: ConfigureState,
  hint = false,
) {
  const id = inputId(field);
  const descriptions = [
    hint ? `${id}${HINT_SUFFIX}` : '',
    state.errors[field] ? `${id}${ERROR_SUFFIX}` : '',
  ]
    .filter(Boolean)
    .join(' ');
  return {
    id,
    'aria-invalid': Boolean(state.errors[field]),
    'aria-describedby': descriptions || undefined,
  };
}

/** @param event Keyboard interaction. @param controller Local state owner. @returns Nothing; Escape cancels, repeat activation is suppressed and Tab stays in this window's form. */
function keyboard(
  event: KeyboardEvent<HTMLFormElement>,
  controller: ConfigureController,
): void {
  if (event.key === KEY.escape) {
    event.preventDefault();
    controller.cancel();
    return;
  }
  if (
    event.repeat &&
    [KEY.enter, KEY.space].some(
      /** @param key Activation key. @returns Whether it matches. */ (key) =>
        key === event.key,
    )
  ) {
    event.preventDefault();
    return;
  }
  if (event.key !== KEY.tab) return;
  const controls =
    event.currentTarget.querySelectorAll<HTMLElement>(ENABLED_CONTROLS);
  const first = controls[FIRST];
  const last = controls[controls.length - LAST_OFFSET];
  const edge = event.shiftKey ? first : last;
  const destination = event.shiftKey ? last : first;
  if (document.activeElement === edge && destination) {
    event.preventDefault();
    destination.focus();
  }
}

/** @param props Draft controller and dismissal callback. @returns Scrollable fields and persistent actions; effects focus actual inputs and release subscriptions on cleanup. */
export function ConfigureForm({
  controller,
  onClosed,
}: {
  controller: ConfigureController;
  onClosed: () => void;
}) {
  const state = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  const editingDisabled = state.pending || state.uncertain;
  const form = useRef<HTMLFormElement>(null);
  const name = useRef<HTMLInputElement>(null);
  const status = useRef<HTMLParagraphElement>(null);
  const save = useRef<HTMLButtonElement>(null);
  const wasPending = useRef(false);
  useLayoutEffect(
    /** Focuses the actual name input once, selecting its text for editing. */ () => {
      name.current?.focus();
      name.current?.select();
    },
    [],
  );
  useLayoutEffect(
    /** Moves focus to validation errors or progress and returns to Save after a nonfield failure. */ () => {
      if (state.focus)
        form.current
          ?.querySelector<HTMLInputElement>(`#${inputId(state.focus)}`)
          ?.focus();
      else if (state.pending) status.current?.focus();
      else if (wasPending.current) save.current?.focus();
      wasPending.current = state.pending;
    },
    [state.focus, state.focusRevision, state.pending],
  );
  useEffect(
    /** Notifies the host only after controller completion; the host owns native window closure. */ () => {
      if (state.closed) onClosed();
    },
    [state.closed, onClosed],
  );
  return (
    <main className="configure-window" aria-labelledby={TITLE_ID}>
      <h1 id={TITLE_ID}>{CONFIGURE_TEXT.title}</h1>
      <form
        ref={form}
        noValidate
        aria-busy={state.pending}
        onKeyDownCapture={
          /** @param event Captured key. @returns Nothing. */ (event) =>
            keyboard(event, controller)
        }
        onSubmit={
          /** @param event Native form submission. @returns Nothing; the controller handles callback failures. */ (
            event,
          ) => {
            event.preventDefault();
            void controller.submit();
          }
        }
      >
        <div className="configure-fields">
          <Field
            field={CONFIGURE_FIELD_KEY.name}
            label={CONFIGURE_TEXT.name}
            state={state}
          >
            <input
              ref={name}
              {...accessibility(CONFIGURE_FIELD_KEY.name, state)}
              type="text"
              autoComplete="off"
              value={state.input.name}
              disabled={editingDisabled}
              onChange={
                /** @param event Edited name. @returns Nothing. */ (event) =>
                  controller.edit(CONFIGURE_FIELD_KEY.name, event.target.value)
              }
              onBlur={
                /** Validates the name after editing. */ () =>
                  controller.blur(CONFIGURE_FIELD_KEY.name)
              }
            />
          </Field>
          <Field
            field={CONFIGURE_FIELD_KEY.speed}
            label={CONFIGURE_TEXT.speed}
            state={state}
          >
            <select
              {...accessibility(CONFIGURE_FIELD_KEY.speed, state)}
              value={state.input.speed}
              disabled={editingDisabled}
              onChange={
                /** @param event Chosen preset. @returns Nothing. */ (event) =>
                  controller.edit(
                    CONFIGURE_FIELD_KEY.speed,
                    Number(event.target.value),
                  )
              }
              onBlur={
                /** Validates the speed selection. */ () =>
                  controller.blur(CONFIGURE_FIELD_KEY.speed)
              }
            >
              {SPEED_PRESETS.map(
                /** @param speed Supported preset. @returns Its accessible option. */ (
                  speed,
                ) => (
                  <option key={speed} value={speed}>
                    {speed}
                    {SPEED_MULTIPLIER_SUFFIX}
                  </option>
                ),
              )}
            </select>
          </Field>
          <Field
            field={CONFIGURE_FIELD_KEY.repeatMode}
            label={CONFIGURE_TEXT.repeatMode}
            state={state}
          >
            <select
              {...accessibility(CONFIGURE_FIELD_KEY.repeatMode, state)}
              value={state.input.repeatMode}
              disabled={editingDisabled}
              onChange={
                /** @param event Chosen mode. @returns Nothing. */ (event) =>
                  controller.edit(
                    CONFIGURE_FIELD_KEY.repeatMode,
                    event.target.value,
                  )
              }
              onBlur={
                /** Validates the mode selection. */ () =>
                  controller.blur(CONFIGURE_FIELD_KEY.repeatMode)
              }
            >
              {Object.values(REPEAT_MODE).map(
                /** @param mode Supported mode. @returns Its labeled option. */ (
                  mode,
                ) => (
                  <option key={mode} value={mode}>
                    {CONFIGURE_TEXT[mode]}
                  </option>
                ),
              )}
            </select>
          </Field>
          <Field
            field={CONFIGURE_FIELD_KEY.totalRuns}
            label={CONFIGURE_TEXT.totalRuns}
            state={state}
            hint={CONFIGURE_TEXT.runsHint}
          >
            <input
              {...accessibility(CONFIGURE_FIELD_KEY.totalRuns, state, true)}
              type="text"
              inputMode="numeric"
              value={state.input.totalRuns}
              disabled={
                editingDisabled || state.input.repeatMode !== REPEAT_MODE.fixed
              }
              onChange={
                /** @param event Edited count. @returns Nothing. */ (event) =>
                  controller.edit(
                    CONFIGURE_FIELD_KEY.totalRuns,
                    event.target.value,
                  )
              }
              onBlur={
                /** Validates the active count. */ () =>
                  controller.blur(CONFIGURE_FIELD_KEY.totalRuns)
              }
            />
          </Field>
          <Field
            field={CONFIGURE_FIELD_KEY.interval}
            label={CONFIGURE_TEXT.interval}
            state={state}
            hint={CONFIGURE_TEXT.intervalHint}
          >
            <input
              {...accessibility(CONFIGURE_FIELD_KEY.interval, state, true)}
              type="text"
              inputMode="decimal"
              value={state.input.interval}
              disabled={
                editingDisabled || state.input.repeatMode === REPEAT_MODE.once
              }
              onChange={
                /** @param event Edited seconds. @returns Nothing. */ (event) =>
                  controller.edit(
                    CONFIGURE_FIELD_KEY.interval,
                    event.target.value,
                  )
              }
              onBlur={
                /** Validates the active interval. */ () =>
                  controller.blur(CONFIGURE_FIELD_KEY.interval)
              }
            />
          </Field>
          {state.message && (
            <p className="configure-error configure-failure" role="alert">
              {state.message}
            </p>
          )}
        </div>
        <footer>
          <p ref={status} role="status" tabIndex={PROGRAMMATIC_TAB_INDEX}>
            {state.pending ? CONFIGURE_TEXT.saving : EMPTY_STATUS}
          </p>
          <button
            type="button"
            disabled={editingDisabled}
            onClick={
              /** Discards idle local edits without submission. */ () => {
                controller.cancel();
              }
            }
          >
            {CONFIGURE_TEXT.cancel}
          </button>
          <button
            ref={save}
            type="submit"
            className="configure-save"
            disabled={state.pending}
          >
            {state.message ? CONFIGURE_TEXT.retry : CONFIGURE_TEXT.save}
          </button>
        </footer>
      </form>
    </main>
  );
}
