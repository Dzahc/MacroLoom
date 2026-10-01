import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './style.css';
import { PrototypeView, type Command, type Snapshot } from './prototype-view';

function App() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [error, setError] = useState('');
  const [uiResponse, setUiResponse] = useState<number | null>(null);

  useEffect(() => {
    let disposed = false;
    const off = listen<Snapshot>('prototype-state', (event) => {
      if (!disposed) setState(event.payload);
    });
    void invoke<Snapshot>('snapshot')
      .then((value) => {
        if (!disposed) setState(value);
      })
      .catch((reason) => setError(String(reason)));
    const timer = window.setInterval(() => {
      void invoke<Snapshot>('snapshot')
        .then((value) => {
          if (!disposed) setState(value);
        })
        .catch(() => {});
    }, 100);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      void off.then((unlisten) => unlisten());
    };
  }, []);

  async function action(command: Command) {
    const start = performance.now();
    setError('');
    try {
      const next = await invoke<Snapshot>(command);
      setState(next);
      requestAnimationFrame(() =>
        setUiResponse(Math.round(performance.now() - start)),
      );
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <PrototypeView
      state={state}
      error={error}
      uiResponse={uiResponse}
      onAction={action}
    />
  );
}

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
