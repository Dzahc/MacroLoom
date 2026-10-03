import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { ActionIcon } from './action-icon';
import {
  ACTION,
  LABEL,
  PHASE,
  SOURCE,
  TOOLBAR,
  TOOLTIP,
  canRequest,
  formatDuration,
  requestAction,
  type ActionName,
  type ActionSource,
  type LibraryCallbacks,
  type LibrarySnapshot,
  type MacroSummary,
} from './library-model';

type ViewProps = LibraryCallbacks & {
  snapshot: LibrarySnapshot;
  notifications?: ReactNode;
};
type MenuAnchor = { macroId: string; x: number; y: number };
const MENU_GAP = 8;
const MENU_ACTIONS = [ACTION.configure, ACTION.delete] as const;
const MENU_EVENT = {
  pointerDown: 'pointerdown',
  blur: 'blur',
  resize: 'resize',
} as const;

/**
 * Sends an available action request to the host without changing library state.
 * @param props Supplied snapshot and host callbacks.
 * @param action Operation to request.
 * @param source Initiating interaction.
 * @param macroId Explicit clicked ID, defaulting to selection.
 * @returns Nothing; unavailable requests are ignored and callback failures propagate.
 */
function emit(
  props: ViewProps,
  action: ActionName,
  source: ActionSource,
  macroId: string | null = props.snapshot.selectedId,
) {
  const request = requestAction(props.snapshot, action, source, macroId);
  if (request) props.onAction(request);
}

/** @param props State/callbacks controlling availability. @returns Icon-only actions with shortcut-aware names. */
function Toolbar(props: ViewProps) {
  return (
    <nav className="library-toolbar" aria-label={LABEL.controls}>
      {TOOLBAR.map(
        /** @param action Ordered toolbar operation. @returns Its icon button and guarded callback. */
        (action) => (
          <button
            type="button"
            key={action}
            className={`icon-button action-${action}`}
            aria-label={TOOLTIP[action]}
            title={TOOLTIP[action]}
            disabled={!canRequest(props.snapshot, action)}
            onClick={
              /** Emits this toolbar action only when available. */ () =>
                emit(props, action, SOURCE.toolbar)
            }
          >
            <ActionIcon action={action} />
          </button>
        ),
      )}
    </nav>
  );
}

/** @param props Supplied status snapshot. @returns A polite text status and fixed shortcut hints. */
function MessageBanner({ snapshot }: { snapshot: LibrarySnapshot }) {
  const busy = snapshot.phase !== PHASE.idle;
  return (
    <section
      className="library-banner"
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      <span className={`status-dot ${busy ? 'busy' : ''}`} aria-hidden="true" />
      <div>
        <strong>{snapshot.message}</strong>
        <span>
          {LABEL.recordShortcut} · {LABEL.stopShortcut}
        </span>
      </div>
    </section>
  );
}

/**
 * Clamps a measured menu to the viewport while retaining an edge gap.
 * @param element Mounted menu whose inline left/top positions are changed.
 * @param anchor Requested pointer coordinates in CSS viewport pixels.
 * @returns Nothing; must run after layout so menu dimensions are available.
 */
function placeMenu(element: HTMLDivElement, anchor: MenuAnchor) {
  const bounds = element.getBoundingClientRect();
  element.style.left = `${Math.max(MENU_GAP, Math.min(anchor.x, window.innerWidth - bounds.width - MENU_GAP))}px`;
  element.style.top = `${Math.max(MENU_GAP, Math.min(anchor.y, window.innerHeight - bounds.height - MENU_GAP))}px`;
}

/**
 * Portals Configure/Delete actions for the clicked macro into the document body.
 * @param props Snapshot/callbacks, pointer anchor, and menu-close callback.
 * @returns A clamped context menu; pointer/blur/resize listeners are removed on cleanup.
 */
function ContextMenu({
  anchor,
  close,
  ...props
}: ViewProps & { anchor: MenuAnchor; close: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(
    /** Measures the mounted menu and clamps it before painting. */
    () => {
      if (ref.current) placeMenu(ref.current, anchor);
    },
    [anchor],
  );
  useEffect(
    /** Registers menu dismissal listeners and returns their cleanup. */
    () => {
      /** @param event Pointer interaction closing the menu when outside its mounted node. @returns Nothing. */
      function outside(event: PointerEvent) {
        if (
          event.target instanceof Node &&
          !ref.current?.contains(event.target)
        )
          close();
      }
      document.addEventListener(MENU_EVENT.pointerDown, outside);
      window.addEventListener(MENU_EVENT.blur, close);
      window.addEventListener(MENU_EVENT.resize, close);
      /** Removes all menu dismissal listeners when the anchor/owner changes or unmounts. */
      return () => {
        document.removeEventListener(MENU_EVENT.pointerDown, outside);
        window.removeEventListener(MENU_EVENT.blur, close);
        window.removeEventListener(MENU_EVENT.resize, close);
      };
    },
    [close],
  );
  return createPortal(
    <div
      ref={ref}
      className="macro-menu"
      role="menu"
      aria-label={LABEL.controls}
      style={{ left: anchor.x, top: anchor.y }}
    >
      {MENU_ACTIONS.map(
        /** @param action Context operation. @returns A menu item using the anchored macro ID. */
        (action) => (
          <button
            type="button"
            role="menuitem"
            key={action}
            disabled={!canRequest(props.snapshot, action, anchor.macroId)}
            onClick={
              /** Emits the anchored request and closes the menu after dispatch. */
              () => {
                emit(props, action, SOURCE.context, anchor.macroId);
                close();
              }
            }
          >
            <ActionIcon action={action} />
            <span>{LABEL[action]}</span>
          </button>
        ),
      )}
    </div>,
    document.body,
  );
}

/**
 * Renders one macro with stable-ID selection and explicit clicked-ID actions.
 * @param props Macro, supplied state/callbacks, and context-menu opener.
 * @returns A selectable row disabled while busy; full names remain accessible.
 */
function MacroRow({
  macro,
  openMenu,
  ...props
}: ViewProps & {
  macro: MacroSummary;
  openMenu: (event: MouseEvent, id: string) => void;
}) {
  const selected = macro.id === props.snapshot.selectedId;
  /** Sends this row's identity/source to the host; does not mutate the supplied snapshot. */
  function select() {
    props.onSelect({ macroId: macro.id, source: SOURCE.row });
  }
  /** Selects this row and emits one available Play request without waiting for selection updates. */
  function play() {
    props.onSelect({ macroId: macro.id, source: SOURCE.doubleClick });
    emit(props, ACTION.play, SOURCE.doubleClick, macro.id);
  }
  return (
    <li>
      <button
        type="button"
        className={`macro-row ${selected ? 'selected' : ''}`}
        aria-pressed={selected}
        disabled={props.snapshot.phase !== PHASE.idle}
        onClick={select}
        onContextMenu={
          /** @param event Context interaction forwarded with this row's ID. @returns Nothing. */ (
            event,
          ) => openMenu(event, macro.id)
        }
        onDoubleClick={play}
        title={macro.name}
      >
        <span className="macro-name">{macro.name}</span>
        <span className="macro-duration">
          {formatDuration(macro.durationMs)}
        </span>
      </button>
    </li>
  );
}

/**
 * Renders the production library from supplied state; owns only transient context-menu state.
 * @param props Snapshot, selection/action callbacks, and optional notification content.
 * @returns Toolbar, status, notifications, and independently scrollable macro list.
 */
export function LibraryView(props: ViewProps) {
  const [menu, setMenu] = useState<MenuAnchor | null>(null);
  /**
   * Selects a valid idle row and opens its menu at the pointer, suppressing the browser menu.
   * @param event Context interaction carrying viewport coordinates.
   * @param macroId Stable identity of the clicked row.
   * @returns Nothing; unavailable requests leave the application menu closed.
   */
  function openMenu(event: MouseEvent, macroId: string) {
    event.preventDefault();
    if (!canRequest(props.snapshot, ACTION.configure, macroId)) return;
    props.onSelect({ macroId, source: SOURCE.context });
    setMenu({ macroId, x: event.clientX, y: event.clientY });
  }
  const menuVisible =
    menu !== null && canRequest(props.snapshot, ACTION.configure, menu.macroId);
  return (
    <main
      className="library-window"
      onScrollCapture={
        /** Closes the anchored menu when library content scrolls. */ () =>
          setMenu(null)
      }
    >
      <Toolbar {...props} />
      <MessageBanner snapshot={props.snapshot} />
      {props.notifications}
      <section className="macro-library" aria-label={LABEL.macros}>
        <div className="library-heading">
          <h1>{LABEL.macros}</h1>
          <span aria-hidden="true">{props.snapshot.macros.length}</span>
        </div>
        {props.snapshot.macros.length === 0 ? (
          <div className="empty-library">
            <ActionIcon action={ACTION.record} />
            <p>{props.snapshot.loading ? LABEL.loading : LABEL.empty}</p>
          </div>
        ) : (
          <ul className="macro-list">
            {props.snapshot.macros.map(
              /** @param macro Valid entry. @returns Its row keyed by stable identity. */
              (macro) => (
                <MacroRow
                  key={macro.id}
                  macro={macro}
                  openMenu={openMenu}
                  {...props}
                />
              ),
            )}
          </ul>
        )}
      </section>
      {menuVisible && (
        <ContextMenu
          {...props}
          anchor={menu}
          close={/** Clears transient menu state. */ () => setMenu(null)}
        />
      )}
    </main>
  );
}
