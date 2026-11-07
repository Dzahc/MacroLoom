import { createElement, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { LibraryView } from '../src/library-view';
import { useNativeConfigure } from '../src/native-configure';
import {
  ACTION,
  LABEL,
  PHASE,
  type LibraryAction,
  type LibrarySnapshot,
  type SelectionChange,
} from '../src/library-model';
import { CONFIGURE_COMMAND, CONFIGURE_EVENT } from '../src/configure-contract';
import { SAMPLE_MACROS } from '../src/sample-macros';

const FIRST = 0;
const EXPECTED_OPENS = 1;
const NO_SELECTIONS = 0;
const NORMAL_TAB_INDEX = 0;
const OPACITY = { hidden: '0', visible: '1' } as const;
const DOUBLE_CLICK = 'dblclick';
const SETTLE_MS = 10;
const MAX_SETTLES = 50;
const ORIGIN = SAMPLE_MACROS[FIRST];
const SELECTOR = {
  shortcut: '[data-configure-id]',
  icon: '.macro-configure-shortcut',
  row: 'button[data-macro-id]',
  toolbar: `.library-toolbar [data-action="${ACTION.configure}"]`,
  outside: '#outside',
  report: '#shortcut-result',
  root: '#root',
} as const;
const SNAPSHOT: LibrarySnapshot = {
  macros: SAMPLE_MACROS,
  selectedId: ORIGIN.id,
  phase: PHASE.idle,
  message: LABEL.ready,
};
const opened: unknown[] = [];
const requests: LibraryAction[] = [];
const failures: string[] = [];
const selections: SelectionChange[] = [];
let foreground = true;

/** @param selector Public UI surface. @returns Its mounted element; a missing control fails this integration. */
function element(selector: string): HTMLElement {
  const target = document.querySelector<HTMLElement>(selector);
  if (!target) throw new Error(`Missing control: ${selector}`);
  return target;
}

/** @param condition Observed public behavior. @param message Failure diagnostic. @returns Nothing; retains every failed observation. */
function check(condition: boolean, message: string): void {
  if (!condition) failures.push(message);
}

/** @returns Completion after queued React/native-boundary promise updates commit. */
async function settle(): Promise<void> {
  await new Promise(
    /** @param resolve Continuation after the current promise queue. @returns Nothing; runs no session or disk effects. */
    (resolve) => window.setTimeout(resolve, SETTLE_MS),
  );
  flushSync(/** Flushes any queued presentation updates. */ () => {});
}

/** @param ready Observable control state. @returns Completion once React/native reconciliation reaches it; a bounded timeout fails the probe. */
async function waitFor(ready: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < MAX_SETTLES; attempt += 1) {
    if (ready()) return;
    await settle();
  }
  throw new Error('Configure controls did not settle');
}

/** @returns The real library and Configure hook, with only native IPC replaced; no private callbacks are invoked by the probe. */
function Fixture() {
  const [selectedId, setSelectedId] = useState<string | null>(ORIGIN.id);
  const configure = useNativeConfigure(
    /** The mocked native editor has no save receipt to apply. */ () => {},
    /** @param reason Native boundary failure. @returns Nothing; makes asynchronous failure observable. */
    (reason) => failures.push(String(reason)),
    /** This fixture has no display-recovery reservation to release. */ () => {},
  );
  return createElement(LibraryView, {
    snapshot: { ...SNAPSHOT, selectedId, confirmationOpen: configure.busy },
    onSelect:
      /** @param change User selection. @returns Nothing; tracks unwanted icon selection and applies real row selection. */
      (change) => {
        selections.push(change);
        setSelectedId(change.macroId);
      },
    onAction:
      /** @param request Guarded UI operation. @returns Nothing; forwards Configure to its production native lifecycle. */
      (request) => {
        requests.push(request);
        if (request.action === ACTION.configure)
          void configure.open(request).catch(
            /** @param reason Mocked creation failure. @returns Nothing; fails the probe instead of leaving an unhandled promise. */
            (reason: unknown) => failures.push(String(reason)),
          );
      },
    onDeleteConfirmed: /** This test never confirms deletion. */ () => {},
  });
}

/** Runs one real opening/dismissal through the UI and native event boundary. @param selector Opening control. @param expectedFocus Control to regain focus. @returns Completion after reconciliation and focus restoration. */
async function openAndDismiss(
  selector: string,
  expectedFocus: HTMLElement,
): Promise<void> {
  const opener = element(selector);
  opener.focus();
  opener.click();
  opener.dispatchEvent(new MouseEvent(DOUBLE_CLICK, { bubbles: true }));
  await waitFor(
    /** @returns Whether the native editor owns the UI interlock. */ () =>
      element(SELECTOR.shortcut).matches(':disabled'),
  );
  check(
    element(SELECTOR.shortcut).matches(':disabled'),
    'Configure must interlock while its editor is open',
  );
  element(SELECTOR.outside).focus();
  await emit(CONFIGURE_EVENT.closed);
  await waitFor(
    /** @returns Whether dismissal reconciliation has released controls. */ () =>
      !element(SELECTOR.shortcut).matches(':disabled'),
  );
  check(
    document.activeElement === expectedFocus,
    `Configure did not restore ${selector}`,
  );
}

/** @returns Probe completion using production React surfaces and mocked Tauri commands/events; never creates a native editor or injects desktop input. */
export async function run(): Promise<void> {
  // Headless Chromium has no OS foreground window; model that external boundary while exercising real element focus.
  Object.defineProperty(document, 'hasFocus', {
    value:
      /** @returns Whether this scenario represents a foreground main window. */ () =>
        foreground,
  });
  mockIPC(
    /** @param command Native command. @param args Explicit native payload. @returns Only Configure lifecycle acknowledgements; all unexpected calls fail. */
    (command, args) => {
      if (command === CONFIGURE_COMMAND.open) {
        opened.push(args);
        return;
      }
      if (command === CONFIGURE_COMMAND.status)
        return { pending: false, completion: null };
      throw new Error(`Unexpected native command: ${command}`);
    },
    { shouldMockEvents: true },
  );
  const root = createRoot(element(SELECTOR.root));
  flushSync(
    /** Mounts the production UI before querying controls. */ () =>
      root.render(createElement(Fixture)),
  );
  await settle();
  const shortcut = element(SELECTOR.shortcut);
  element(SELECTOR.outside).focus();
  check(
    getComputedStyle(element(SELECTOR.icon)).opacity === OPACITY.hidden,
    'Configure must stay hidden without row hover or keyboard focus',
  );
  element(SELECTOR.row).focus();
  check(
    getComputedStyle(element(SELECTOR.icon)).opacity === OPACITY.visible,
    'Keyboard row focus must reveal Configure',
  );
  check(
    shortcut.tabIndex === NORMAL_TAB_INDEX,
    'Enabled Configure must remain in normal Tab order',
  );
  await openAndDismiss(SELECTOR.shortcut, shortcut);
  check(
    opened.length === EXPECTED_OPENS,
    'Shortcut must open exactly one Configure editor',
  );
  check(
    JSON.stringify(opened[FIRST]) === JSON.stringify({ macroId: ORIGIN.id }),
    'Shortcut must configure the selected macro',
  );
  check(
    selections.length === NO_SELECTIONS,
    'Shortcut must not change selection',
  );
  check(
    requests.every(
      /** @param request Emitted UI action. @returns Whether activation avoided playback. */ (
        request,
      ) => request.action === ACTION.configure,
    ),
    'Shortcut must not request Play',
  );
  await openAndDismiss(SELECTOR.toolbar, element(SELECTOR.toolbar));
  foreground = false;
  await openAndDismiss(SELECTOR.shortcut, element(SELECTOR.outside));
  foreground = true;
  element(SELECTOR.row).focus();
  element(SELECTOR.row).click();
  await settle();
  check(
    document.activeElement === element(SELECTOR.row),
    'Skipped restoration must not replay after foreground focus returns',
  );
  root.unmount();
  element(SELECTOR.report).textContent = JSON.stringify({ failures });
}
