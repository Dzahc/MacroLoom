import React, { lazy, Suspense, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { LibraryApp } from './library-app';
import './style.css';
import './configure.css';
import { CONFIGURE_QUERY } from './configure-contract';
import { NativeConfigureApp } from './native-configure-app';
import { ConfigureSample } from './configure-sample';

type AppMode = { inputPrototype: boolean };
const LIBRARY_MODE: AppMode = { inputPrototype: false };
const MODE_COMMAND = 'app_mode';
const ROOT_ID = 'root';
const STARTING = 'Starting MacroLoom…';
const START_FAILED = 'Could not start MacroLoom.';
const PrototypeApp = lazy(
  /** @returns The optional prototype module promise; loading failures propagate to React. */
  () =>
    import('./prototype-app').then(
      /** @param module Loaded prototype module. @returns React's lazy default-export shape. */
      (module) => ({
        default: module.PrototypeApp,
      }),
    ),
);

/**
 * Resolves typed native launch metadata and selects the library or explicit prototype.
 * @returns Loading/error presentation until mode resolves, then the selected application.
 * Pending mode results are ignored after unmount; browser previews use library mode directly.
 */
function App() {
  const [mode, setMode] = useState<AppMode | null>(
    /** @returns Browser library mode, or null while native launch metadata is pending. */
    () => (isTauri() ? null : LIBRARY_MODE),
  );
  const [error, setError] = useState('');
  useEffect(
    /** Resolves native mode once and returns cleanup suppressing post-unmount updates. */
    () => {
      if (!isTauri()) return;
      let disposed = false;
      void invoke<AppMode>(MODE_COMMAND)
        .then(
          /** @param value Typed native mode applied only to the mounted app. @returns Nothing. */
          (value) => {
            if (!disposed) setMode(value);
          },
        )
        .catch(
          /** @param reason Mode-command failure reported while mounted. @returns Nothing. */
          (reason: unknown) => {
            if (!disposed) setError(`${START_FAILED} ${String(reason)}`);
          },
        );
      /** Marks the pending request disposed; does not cancel backend work. */
      return () => {
        disposed = true;
      };
    },
    [],
  );
  if (error) return <p role="alert">{error}</p>;
  if (!mode) return <p role="status">{STARTING}</p>;
  if (mode.inputPrototype)
    return (
      <Suspense fallback={<p role="status">{STARTING}</p>}>
        <PrototypeApp />
      </Suspense>
    );
  return <LibraryApp />;
}

/** @returns The owned editor route or main application; browser editor routes use nonmutating controlled callbacks. */
function RoutedApp() {
  if (new URLSearchParams(window.location.search).has(CONFIGURE_QUERY))
    return isTauri() ? <NativeConfigureApp /> : <ConfigureSample />;
  return <App />;
}

createRoot(document.getElementById(ROOT_ID)!).render(
  <React.StrictMode>
    <RoutedApp />
  </React.StrictMode>,
);
