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
      {idle && <p>{SESSION_TEXT.playbackGuidance}</p>}
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
      <SaveControls snapshot={snapshot} controller={controller} />
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

/** @param props Retained native save and command owner. @returns Recovery with explicit exit intent reflected in each action. */
function SaveControls({
  snapshot,
  controller,
}: {
  snapshot: SessionSnapshot;
  controller: SessionController;
}) {
  if (!snapshot.saveFailed) return null;
  return (
    <>
      <p role="alert">{snapshot.message}</p>
      <button
        type="button"
        onClick={
          /** Retries the same retained macro; pending exit is owned by the backend. */
          () => {
            void controller.request(SESSION_COMMAND.retryRecording);
          }
        }
      >
        {snapshot.closeRequested
          ? SESSION_TEXT.retryExit
          : SESSION_TEXT.retryRecording}
      </button>
      {!snapshot.closeRequested && (
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
      )}
    </>
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
      {snapshot.closeRequested &&
        snapshot.unsaved &&
        !snapshot.cleanupFailed &&
        (idle || snapshot.saveFailed) && (
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
