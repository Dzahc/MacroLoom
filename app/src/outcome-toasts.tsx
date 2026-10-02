import { useEffect, useSyncExternalStore } from 'react';
import { ActionIcon } from './action-icon';
import { LABEL } from './library-model';
import { OutcomeQueue, PAUSE } from './outcome-queue';

export function OutcomeToasts({ queue }: { queue: OutcomeQueue }) {
  const outcomes = useSyncExternalStore(
    queue.subscribe,
    queue.getSnapshot,
    queue.getSnapshot,
  );
  const current = outcomes[0];
  useEffect(() => {
    function visibility() {
      if (document.hidden) queue.pause(PAUSE.hidden);
      else queue.resume(PAUSE.hidden);
    }
    visibility();
    document.addEventListener('visibilitychange', visibility);
    return () => document.removeEventListener('visibilitychange', visibility);
  }, [queue]);
  return (
    <div className="toast-slot">
      <div
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {current && <span key={current.id}>{current.message}</span>}
      </div>
      {current && (
        <aside
          key={current.id}
          className={`outcome-toast ${current.kind}`}
          onMouseEnter={() => queue.pause(PAUSE.hover)}
          onMouseLeave={() => queue.resume(PAUSE.hover)}
          onFocusCapture={() => queue.pause(PAUSE.focus)}
          onBlurCapture={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget))
              queue.resume(PAUSE.focus);
          }}
        >
          <ActionIcon action={current.kind} />
          <p>{current.message}</p>
          <button
            type="button"
            className="icon-button"
            aria-label={LABEL.dismiss}
            title={LABEL.dismiss}
            onClick={queue.dismiss}
          >
            <ActionIcon action="close" />
          </button>
        </aside>
      )}
    </div>
  );
}
