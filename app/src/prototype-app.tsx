import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PrototypeView, type Command, type Snapshot } from './prototype-view';
import './prototype.css';

const SNAPSHOT_COMMAND = 'snapshot';
const STATE_EVENT = 'prototype-state';
const POLL_MS = 100;

export function PrototypeApp() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [error, setError] = useState('');
  const [uiResponse, setUiResponse] = useState<number | null>(null);
  useEffect(() => {
    let disposed = false;
    function failed(reason: unknown) {
      if (!disposed) setError(String(reason));
    }
    function receive(value: Snapshot) {
      if (!disposed) setState(value);
    }
    const off = listen<Snapshot>(STATE_EVENT, (event) =>
      receive(event.payload),
    );
    void off.catch(failed);
    function refresh() {
      void invoke<Snapshot>(SNAPSHOT_COMMAND).then(receive).catch(failed);
    }
    refresh();
    const timer = window.setInterval(refresh, POLL_MS);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      void off.then((unlisten) => unlisten()).catch(failed);
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
    <div className="input-prototype">
      <PrototypeView
        state={state}
        error={error}
        uiResponse={uiResponse}
        onAction={action}
      />
    </div>
  );
}
