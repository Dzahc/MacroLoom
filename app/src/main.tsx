import React, { lazy, Suspense, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { LibraryApp } from './library-app';
import './style.css';

type AppMode = { inputPrototype: boolean };
const LIBRARY_MODE: AppMode = { inputPrototype: false };
const MODE_COMMAND = 'app_mode';
const ROOT_ID = 'root';
const STARTING = 'Starting MacroLoom…';
const START_FAILED = 'Could not start MacroLoom.';
const PrototypeApp = lazy(() =>
  import('./prototype-app').then((module) => ({
    default: module.PrototypeApp,
  })),
);

function App() {
  const [mode, setMode] = useState<AppMode | null>(() =>
    isTauri() ? null : LIBRARY_MODE,
  );
  const [error, setError] = useState('');
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    void invoke<AppMode>(MODE_COMMAND)
      .then((value) => {
        if (!disposed) setMode(value);
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(`${START_FAILED} ${String(reason)}`);
      });
    return () => {
      disposed = true;
    };
  }, []);
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

createRoot(document.getElementById(ROOT_ID)!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
