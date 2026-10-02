import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PrototypeView, type Command, type Snapshot } from './prototype-view';
import './prototype.css';

const SNAPSHOT_COMMAND = 'snapshot';
const STATE_EVENT = 'prototype-state';
const POLL_MS = 100;

/**
 * Adapts the optional live prototype's typed commands/events to presentation state.
 * @returns Prototype UI with timing feedback and asynchronous failure messages.
 * Poll timers/listeners are released on unmount and subscription results are then ignored.
 */
export function PrototypeApp() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [error, setError] = useState('');
  const [uiResponse, setUiResponse] = useState<number | null>(null);
  useEffect(
    /** Subscribes to state, polls snapshots, and returns timer/listener cleanup. */
    () => {
      let disposed = false;
      /** @param reason Async command/subscription failure shown only while mounted. @returns Nothing. */
      function failed(reason: unknown) {
        if (!disposed) setError(String(reason));
      }
      /** @param value Backend snapshot applied only while this subscription is mounted. @returns Nothing. */
      function receive(value: Snapshot) {
        if (!disposed) setState(value);
      }
      const off = listen<Snapshot>(
        STATE_EVENT,
        /** @param event Typed state event. @returns Nothing; forwards its snapshot while mounted. */
        (event) => receive(event.payload),
      );
      void off.catch(failed);
      /** Requests a typed snapshot without blocking; results/errors use mount-aware handlers. */
      function refresh() {
        void invoke<Snapshot>(SNAPSHOT_COMMAND).then(receive).catch(failed);
      }
      refresh();
      const timer = window.setInterval(refresh, POLL_MS);
      /** Stops polling and releases the asynchronously registered event listener after unmount. */
      return () => {
        disposed = true;
        window.clearInterval(timer);
        void off
          .then(
            /** @param unlisten Registered listener's cleanup. @returns Nothing; removes the listener. */
            (unlisten) => unlisten(),
          )
          .catch(failed);
      };
    },
    [],
  );
  /**
   * Executes a live command and measures response through the next animation frame.
   * @param command Typed native command; backend enforces session/mode preconditions.
   * @returns Promise resolving after command handling; failures update UI error state.
   */
  async function action(command: Command) {
    const start = performance.now();
    setError('');
    try {
      const next = await invoke<Snapshot>(command);
      setState(next);
      requestAnimationFrame(
        /** Records command-to-next-frame elapsed milliseconds as response feedback. */
        () => setUiResponse(Math.round(performance.now() - start)),
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
