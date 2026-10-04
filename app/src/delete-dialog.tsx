import {
  useEffectEvent,
  useId,
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { DELETE_TEXT, type DeleteConfirmation } from './delete-confirmation';
import { ACTION, PHASE, SOURCE, type LibrarySnapshot } from './library-model';

const FOCUS_SELECTOR = {
  delete: `[data-action="${ACTION.delete}"]`,
  rows: '[data-macro-id]:not(:disabled)',
  selected: '[data-macro-id][aria-pressed="true"]:not(:disabled)',
  fallback: '.macro-library',
} as const;
export const ACTIVATION_KEY = { enter: 'Enter', space: ' ' } as const;
const TAB_KEY = 'Tab';
const ENABLED_BUTTONS = 'button:not(:disabled)';
const FIRST_BUTTON = 0;
const LAST_BUTTON_OFFSET = 1;

type Props = {
  attempt: DeleteConfirmation;
  snapshot: LibrarySnapshot;
  onCancel: (event?: MouseEvent<HTMLButtonElement>) => void;
  onConfirm: (event: MouseEvent<HTMLButtonElement>) => void;
};

/**
 * Suppresses held activation keys before the browser dispatches a button click.
 * @param event Keyboard event from the dialog or its library owner.
 * @returns Nothing; ordinary initial Enter/Space retain native button behavior.
 */
export function suppressRepeatedActivation(event: KeyboardEvent): void {
  if (
    event.repeat &&
    (event.key === ACTIVATION_KEY.enter || event.key === ACTIVATION_KEY.space)
  ) {
    event.preventDefault();
    event.stopPropagation();
  }
}

/**
 * Wraps sequential focus at the modal's edges; WebView2 otherwise allows Tab to leave its web content.
 * @param event Dialog keyboard event; Enter/Space repeats retain the owner's suppression policy.
 * @returns Nothing; wrapped focus scrolls into view and ordinary navigation between controls remains native.
 */
function containKeyboardFocus(event: KeyboardEvent<HTMLDialogElement>): void {
  suppressRepeatedActivation(event);
  if (event.key !== TAB_KEY) return;
  const buttons =
    event.currentTarget.querySelectorAll<HTMLButtonElement>(ENABLED_BUTTONS);
  const first = buttons[FIRST_BUTTON];
  const last = buttons[buttons.length - LAST_BUTTON_OFFSET];
  const edge = event.shiftKey ? first : last;
  const destination = event.shiftKey ? last : first;
  if (document.activeElement === edge && destination) {
    event.preventDefault();
    destination.focus();
  }
}

/**
 * Finds the launch control or an available library fallback without interpolating arbitrary IDs into selectors.
 * @param owner Mounted library root.
 * @param attempt Frozen origin and identity.
 * @returns A usable focus target, or null if the owner is unavailable.
 */
function returnTarget(
  owner: HTMLElement,
  attempt: DeleteConfirmation,
): HTMLElement | null {
  if (attempt.source === SOURCE.toolbar) {
    const toolbar = owner.querySelector<HTMLButtonElement>(
      FOCUS_SELECTOR.delete,
    );
    if (toolbar && !toolbar.disabled) return toolbar;
  }
  const row = Array.from(
    owner.querySelectorAll<HTMLButtonElement>(FOCUS_SELECTOR.rows),
  ).find(
    /** @param element Enabled macro row. @returns Whether it is the launching row. */
    (element) => element.dataset.macroId === attempt.target.id,
  );
  return (
    row ??
    owner.querySelector<HTMLElement>(FOCUS_SELECTOR.selected) ??
    owner.querySelector<HTMLElement>(FOCUS_SELECTOR.rows) ??
    owner.querySelector<HTMLElement>(FOCUS_SELECTOR.fallback)
  );
}

/**
 * Restores focus after the owner commits dismissal, using its current phase rather than an unmounted dialog's state.
 * @param owner Still-mounted library root, or null during application teardown.
 * @param attempt Dismissed attempt's stable origin.
 * @param snapshot Current library state; active sessions take precedence over restoration.
 * @returns Nothing; never activates a background document or scrolls the library.
 */
export function restoreDeleteFocus(
  owner: HTMLElement | null,
  attempt: DeleteConfirmation,
  snapshot: LibrarySnapshot,
): void {
  if (snapshot.phase === PHASE.idle && document.hasFocus() && owner) {
    returnTarget(owner, attempt)?.focus({ preventScroll: true });
  }
}

/**
 * Presents an owned HTML modal with native background inertness and focus containment.
 * @param props Frozen attempt, latest state, and explicit confirmation/cancellation callbacks.
 * @returns A body portal; layout cleanup closes the modal before the owner restores idle focus.
 * Known invalidation cancels before paint. Backdrop interaction has no dismissal handler.
 */
export function DeleteDialog({
  attempt,
  snapshot,
  onCancel,
  onConfirm,
}: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const title = useId();
  const description = useId();
  const available = attempt.isAvailable(snapshot);
  const cancelUnavailable = useEffectEvent(
    /** Cancels the stale attempt using the current owner callback. */
    () => onCancel(),
  );
  useLayoutEffect(
    /** Opens once per mounted attempt and scrolls Cancel into visible focus; releases the native modal on cleanup. */
    () => {
      const element = dialog.current;
      if (!element) return;
      element.showModal();
      cancel.current?.focus();
      /** Closes before the owner chooses a stable return target; cleanup never emits confirmation. */
      return () => {
        element.close();
      };
    },
    [],
  );
  useLayoutEffect(
    /** Prevents confirmation when a known library or phase update invalidates the displayed target. */
    () => {
      if (!available) cancelUnavailable();
    },
    [available],
  );
  return createPortal(
    <dialog
      ref={dialog}
      className="delete-dialog"
      aria-modal="true"
      aria-labelledby={title}
      aria-describedby={description}
      onKeyDownCapture={containKeyboardFocus}
      onCancel={
        /** @param event Native Escape cancellation, delegated to the owner without confirmation. */
        (event) => {
          event.preventDefault();
          onCancel();
        }
      }
    >
      <header>
        <h2 id={title}>{DELETE_TEXT.title}</h2>
        <button
          type="button"
          className="delete-dialog-close"
          aria-label={DELETE_TEXT.close}
          onClick={onCancel}
        >
          <span aria-hidden="true">{DELETE_TEXT.closeSymbol}</span>
        </button>
      </header>
      <p id={description}>
        {DELETE_TEXT.warningStart}
        <strong>{attempt.target.name}</strong>
        {DELETE_TEXT.warningEnd}
      </p>
      <footer>
        <button type="button" ref={cancel} onClick={onCancel}>
          {DELETE_TEXT.cancel}
        </button>
        <button
          type="button"
          className="delete-confirm"
          disabled={!available}
          onClick={onConfirm}
        >
          {DELETE_TEXT.confirm}
        </button>
      </footer>
    </dialog>,
    document.body,
  );
}
