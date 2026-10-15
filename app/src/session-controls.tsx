import { invoke } from '@tauri-apps/api/core';
import { PHASE } from './library-model';
import {
  SESSION_COMMAND,
  SESSION_TEXT,
  type SessionSnapshot,
} from './session-contract';
import type { SessionController } from './session-controller';

/** @param props Authoritative native status and command owner. @returns Nonmodal recovery and explicit close controls; starts no simulated sessions. */
export function SessionControls({
  snapshot,
  controller,
}: {
  snapshot: SessionSnapshot;
  controller: SessionController;
}) {
  const idle = snapshot.phase === PHASE.idle;
  return (
    <div className="session-controls">
      {idle &&
        snapshot.unsaved &&
        snapshot.message !== SESSION_TEXT.retained && (
          <p>{SESSION_TEXT.retained}</p>
        )}
      {snapshot.cleanupFailed && (
        <>
          <p role="alert">{snapshot.message}</p>
          <button
            type="button"
            onClick={
              /** Retries only outstanding native cleanup through the ordinary Stop workflow. */
              () => {
                void controller.request(SESSION_COMMAND.stop);
              }
            }
          >
            {SESSION_TEXT.cleanup}
          </button>
        </>
      )}
      {snapshot.saveFailed && (
        <>
          <p role="alert">{snapshot.message}</p>
          <button
            type="button"
            onClick={
              /** Retries the same retained macro; success remains backend-authoritative. */
              () => {
                void controller.request(SESSION_COMMAND.retryRecording);
              }
            }
          >
            {SESSION_TEXT.retryRecording}
          </button>
          <button
            type="button"
            onClick={
              /** Explicitly abandons only this failed recording; committed files stay intact. */
              () => {
                void controller.request(SESSION_COMMAND.discardRecording);
              }
            }
          >
            {SESSION_TEXT.discardRecording}
          </button>
        </>
      )}
      {idle && (!snapshot.shortcuts.record || !snapshot.shortcuts.stop) && (
        <button
          type="button"
          onClick={
            /** Rechecks OS shortcut ownership without starting recording or playback. */
            () => {
              void controller.request(SESSION_COMMAND.retry);
            }
          }
        >
          {SESSION_TEXT.retry}
        </button>
      )}
      <CloseControls snapshot={snapshot} controller={controller} />
    </div>
  );
}

/** @param props Backend close state and command owner. @returns Explicit close recovery after cleanup or a failed save. */
function CloseControls({
  snapshot,
  controller,
}: {
  snapshot: SessionSnapshot;
  controller: SessionController;
}) {
  const idle = snapshot.phase === PHASE.idle;
  return (
    <>
      {snapshot.closeRequested && (idle || snapshot.saveFailed) && (
        <>
          <p role="alert">{SESSION_TEXT.close}</p>
          <button
            type="button"
            onClick={
              /** Cancels exit while retaining captured input. */
              () => {
                void controller.request(SESSION_COMMAND.keepOpen);
              }
            }
          >
            {SESSION_TEXT.keepOpen}
          </button>
          <button
            type="button"
            disabled={!idle && !snapshot.saveFailed}
            onClick={
              /** Explicitly discards unsaved capture; backend permits exit only after cleanup. */
              () => {
                void invoke<void>(SESSION_COMMAND.discardExit).catch(
                  /** @param reason Failed close/cleanup. @returns Nothing; refreshes native state without claiming exit. */
                  (reason: unknown) => {
                    console.error(SESSION_TEXT.failure, reason);
                    void controller.request(SESSION_COMMAND.snapshot);
                  },
                );
              }
            }
          >
            {SESSION_TEXT.discard}
          </button>
        </>
      )}
    </>
  );
}
