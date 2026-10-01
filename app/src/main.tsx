import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './style.css';

type Snapshot = {
  phase: 'idle' | 'recording' | 'playing';
  eventCount: number;
  dragMoveCount: number;
  lastRelease: [number, number] | null;
  lastReplayRelease: [number, number] | null;
  durationMs: number;
  message: string;
  captureGapMinMs: number | null;
  dispatchLatenessP95Ms: number | null;
  dispatchLatenessMaxMs: number | null;
  stopResponseMs: number | null;
  f9Available: boolean;
  f8Available: boolean;
};

function App() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [error, setError] = useState('');
  const [uiResponse, setUiResponse] = useState<number | null>(null);

  useEffect(() => {
    let disposed = false;
    const off = listen<Snapshot>('prototype-state', event => {
      if (!disposed) setState(event.payload);
    });
    invoke<Snapshot>('snapshot').then(value => {
      if (!disposed) setState(value);
    }).catch(reason => setError(String(reason)));
    const timer = window.setInterval(() => {
      invoke<Snapshot>('snapshot').then(value => {
        if (!disposed) setState(value);
      }).catch(() => {});
    }, 100);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      void off.then(unlisten => unlisten());
    };
  }, []);

  async function action(command: 'start_recording' | 'stop' | 'play') {
    const start = performance.now();
    setError('');
    try {
      const next = await invoke<Snapshot>(command);
      setState(next);
      requestAnimationFrame(() => setUiResponse(Math.round(performance.now() - start)));
    } catch (reason) {
      setError(String(reason));
    }
  }

  const phase = state?.phase ?? 'idle';
  const active = phase !== 'idle';
  return <main className={active ? 'active' : ''}>
    <header>
      <div className="brand"><span className="logo">M</span><div><strong>MacroLoom</strong><small>Windows input prototype</small></div></div>
      <span className="prototype">ML-8</span>
    </header>
    <nav aria-label="Prototype controls">
      <button onClick={() => void action('start_recording')} disabled={active || !state?.f8Available}>● <span>Record</span><kbd>F9</kbd></button>
      <button onClick={() => void action('play')} disabled={active || !state?.eventCount || !state?.f8Available}>▶ <span>Play</span></button>
      <button onClick={() => void action('stop')} disabled={!active}>■ <span>Stop</span><kbd>F8</kbd></button>
    </nav>
    <section className={`banner ${phase}`} role="status" aria-live="polite">
      <span className="dot" />
      <div><strong>{phase === 'recording' ? 'Recording' : phase === 'playing' ? 'Playing' : 'Ready'}</strong><p>{error || state?.message || 'Starting…'}</p></div>
      {active && <time>{((state?.durationMs ?? 0) / 1000).toFixed(1)}s</time>}
    </section>
    {!active && <section className="content">
      <h2>In-memory take</h2>
      <p className="count">{state?.eventCount ?? 0} events · {((state?.durationMs ?? 0) / 1000).toFixed(1)}s</p>
      <p>Focus another ordinary app, press F9, then click its input field and type or drag. Press F8 to stop. Return the target to the same screen position before Play.</p>
      <div className="metrics">
        <div><span>Drag moves</span><strong>{state?.dragMoveCount ?? 0}</strong></div>
        <div><span>Release pixel: record / replay</span><strong>{state?.lastRelease?.join(', ') ?? '—'} / {state?.lastReplayRelease?.join(', ') ?? '—'}</strong></div>
        <div><span>Capture minimum gap</span><strong>{state?.captureGapMinMs ?? '—'} ms</strong></div>
        <div><span>Dispatch lateness p95 / max</span><strong>{state?.dispatchLatenessP95Ms ?? '—'} / {state?.dispatchLatenessMaxMs ?? '—'} ms</strong></div>
        <div><span>Stop response</span><strong>{state?.stopResponseMs ?? '—'} ms</strong></div>
        <div><span>Last UI response</span><strong>{uiResponse ?? '—'} ms</strong></div>
      </div>
      <p className="footnote">Prototype data disappears when this app closes. Physical input uses the shared Windows cursor and keyboard focus.</p>
    </section>}
  </main>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
