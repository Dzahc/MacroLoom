import { useEffect, useLayoutEffect, useRef, type KeyboardEvent } from 'react';
import { ActionIcon } from './action-icon';
import { RENAME_KEY, RENAME_TEXT, type RenameBinding } from './rename-contract';

const POINTER_DOWN = 'pointerdown';
const INPUT_TYPE = 'text';

/** @param props Controlled name draft and persistence callbacks. @returns An inline accessible field with explicit boxed Save/Cancel controls. */
export function RenameEditor({ rename }: { rename: RenameBinding }) {
  const group = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const { snapshot, cancel } = rename;
  const errorId = `rename-error-${snapshot.draft?.macroId}`;
  useLayoutEffect(
    /** Selects the original name once when the row editor mounts. */ () => {
      if (!document.hasFocus()) return;
      input.current?.focus({ preventScroll: true });
      input.current?.select();
    },
    [],
  );
  useEffect(
    /** Installs in-app dismissal without preventing the clicked target's action or listening for application blur. */ () => {
      /** @param event Pointer destination. @returns Nothing; internal Save/Cancel clicks precede focus loss. */
      function outside(event: PointerEvent) {
        if (
          event.target instanceof Node &&
          !group.current?.contains(event.target)
        )
          cancel(false);
      }
      document.addEventListener(POINTER_DOWN, outside, true);
      /** Releases the owned document listener. */
      return () => document.removeEventListener(POINTER_DOWN, outside, true);
    },
    [cancel],
  );
  /** @param event Editor keyboard gesture. @returns Nothing; IME composition and ordinary Tab navigation remain native. */
  function key(event: KeyboardEvent<HTMLDivElement>) {
    if (event.nativeEvent.isComposing) return;
    if (event.key === RENAME_KEY.cancel) {
      event.preventDefault();
      event.stopPropagation();
      cancel();
    }
    if (event.key === RENAME_KEY.save && event.target === input.current) {
      event.preventDefault();
      event.stopPropagation();
      void rename.save();
    }
  }
  return (
    <div
      className="macro-rename"
      ref={group}
      onKeyDown={key}
      aria-busy={snapshot.pending}
      onBlur={
        /** @param event Focus transition; null destinations retain drafts across application switches. */
        (event) => {
          if (
            event.relatedTarget instanceof Node &&
            !event.currentTarget.contains(event.relatedTarget)
          )
            cancel(false);
        }
      }
    >
      <div className="macro-rename-controls">
        <input
          ref={input}
          type={INPUT_TYPE}
          aria-label={RENAME_TEXT.name}
          aria-invalid={Boolean(snapshot.error)}
          aria-describedby={snapshot.error ? errorId : undefined}
          value={snapshot.draft?.name ?? ''}
          disabled={snapshot.pending}
          onChange={
            /** @param event Current untrimmed field. */ (event) =>
              rename.change(event.target.value)
          }
        />
        <button
          type="button"
          className="rename-button"
          aria-label={snapshot.uncertain ? RENAME_TEXT.retry : RENAME_TEXT.save}
          title={snapshot.uncertain ? RENAME_TEXT.retry : RENAME_TEXT.save}
          disabled={snapshot.pending && !snapshot.uncertain}
          onClick={
            /** Submits once or reconciles an uncertain native outcome. */ () => {
              void rename.save();
            }
          }
        >
          <ActionIcon action="success" />
        </button>
        <button
          type="button"
          className="rename-button"
          aria-label={RENAME_TEXT.cancel}
          title={RENAME_TEXT.cancel}
          disabled={snapshot.pending}
          onClick={
            /** Cancels an unsubmitted draft and restores its row. */ () =>
              cancel()
          }
        >
          <ActionIcon action="close" />
        </button>
      </div>
      {snapshot.error && (
        <span id={errorId} className="rename-error" role="alert">
          {snapshot.error}
        </span>
      )}
    </div>
  );
}
