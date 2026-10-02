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

function emit(
  props: ViewProps,
  action: ActionName,
  source: ActionSource,
  macroId: string | null = props.snapshot.selectedId,
) {
  const request = requestAction(props.snapshot, action, source, macroId);
  if (request) props.onAction(request);
}

function Toolbar(props: ViewProps) {
  return (
    <nav className="library-toolbar" aria-label={LABEL.controls}>
      {TOOLBAR.map((action) => (
        <button
          type="button"
          key={action}
          className={`icon-button action-${action}`}
          aria-label={LABEL[action]}
          title={TOOLTIP[action]}
          disabled={!canRequest(props.snapshot, action)}
          onClick={() => emit(props, action, SOURCE.toolbar)}
        >
          <ActionIcon action={action} />
        </button>
      ))}
    </nav>
  );
}

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

function placeMenu(element: HTMLDivElement, anchor: MenuAnchor) {
  const bounds = element.getBoundingClientRect();
  element.style.left = `${Math.max(MENU_GAP, Math.min(anchor.x, window.innerWidth - bounds.width - MENU_GAP))}px`;
  element.style.top = `${Math.max(MENU_GAP, Math.min(anchor.y, window.innerHeight - bounds.height - MENU_GAP))}px`;
}

function ContextMenu({
  anchor,
  close,
  ...props
}: ViewProps & { anchor: MenuAnchor; close: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (ref.current) placeMenu(ref.current, anchor);
  }, [anchor]);
  useEffect(() => {
    function outside(event: PointerEvent) {
      if (event.target instanceof Node && !ref.current?.contains(event.target))
        close();
    }
    document.addEventListener('pointerdown', outside);
    window.addEventListener('blur', close);
    window.addEventListener('resize', close);
    return () => {
      document.removeEventListener('pointerdown', outside);
      window.removeEventListener('blur', close);
      window.removeEventListener('resize', close);
    };
  }, [close]);
  return createPortal(
    <div
      ref={ref}
      className="macro-menu"
      role="menu"
      aria-label={LABEL.controls}
      style={{ left: anchor.x, top: anchor.y }}
    >
      {MENU_ACTIONS.map((action) => (
        <button
          type="button"
          role="menuitem"
          key={action}
          disabled={!canRequest(props.snapshot, action, anchor.macroId)}
          onClick={() => {
            emit(props, action, SOURCE.context, anchor.macroId);
            close();
          }}
        >
          <ActionIcon action={action} />
          <span>{LABEL[action]}</span>
        </button>
      ))}
    </div>,
    document.body,
  );
}

function MacroRow({
  macro,
  openMenu,
  ...props
}: ViewProps & {
  macro: MacroSummary;
  openMenu: (event: MouseEvent, id: string) => void;
}) {
  const selected = macro.id === props.snapshot.selectedId;
  function select() {
    props.onSelect({ macroId: macro.id, source: SOURCE.row });
  }
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
        onContextMenu={(event) => openMenu(event, macro.id)}
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

export function LibraryView(props: ViewProps) {
  const [menu, setMenu] = useState<MenuAnchor | null>(null);
  function openMenu(event: MouseEvent, macroId: string) {
    event.preventDefault();
    if (!canRequest(props.snapshot, ACTION.configure, macroId)) return;
    props.onSelect({ macroId, source: SOURCE.context });
    setMenu({ macroId, x: event.clientX, y: event.clientY });
  }
  const menuVisible =
    menu !== null && canRequest(props.snapshot, ACTION.configure, menu.macroId);
  return (
    <main className="library-window" onScrollCapture={() => setMenu(null)}>
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
            <p>{LABEL.empty}</p>
          </div>
        ) : (
          <ul className="macro-list">
            {props.snapshot.macros.map((macro) => (
              <MacroRow
                key={macro.id}
                macro={macro}
                openMenu={openMenu}
                {...props}
              />
            ))}
          </ul>
        )}
      </section>
      {menuVisible && (
        <ContextMenu {...props} anchor={menu} close={() => setMenu(null)} />
      )}
    </main>
  );
}
