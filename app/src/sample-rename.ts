import { useEffect, useState, useSyncExternalStore } from 'react';
import { RenameController } from './rename-controller';
import { PHASE, type LibraryPhase, type MacroSummary } from './library-model';
import { RENAME_INITIAL_ATTEMPT, type RenameBinding } from './rename-contract';

/** @param macros Controlled scenario rows. @param phase Scenario availability. @returns In-memory rename preview with the production editor and no native input or disk effects. */
export function useSampleRename(
  macros: readonly MacroSummary[],
  phase: LibraryPhase,
) {
  const [names, setNames] = useState<Readonly<Record<string, string>>>({});
  const [controller] = useState(
    /** Creates an isolated preview transport whose names never reach disk. */ () => {
      let attemptId = RENAME_INITIAL_ATTEMPT;
      const current = new Map(
        macros.map(
          /** @param macro Original preview row. @returns Its stable name association. */ (
            macro,
          ) => [macro.id, macro.name],
        ),
      );
      return new RenameController({
        /** @returns The last simulated receipt sequence. */
        status: () =>
          Promise.resolve({ attemptId, pending: false, completion: null }),
        /** @param input Validated name-only preview. @returns Immediate simulated commit for this browser sample. */
        submit: (input) => {
          const changed = current.get(input.macroId) !== input.name;
          attemptId = input.attemptId;
          current.set(input.macroId, input.name);
          setNames(
            /** @param previous Existing preview overrides. @returns Updated display metadata only. */ (
              previous,
            ) => ({ ...previous, [input.macroId]: input.name }),
          );
          return Promise.resolve({
            attemptId,
            result: { ok: true, changed, name: input.name },
            library: null,
            discarded: false,
          });
        },
      });
    },
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  useEffect(
    /** Cancels sample drafts when the externally selected scenario becomes busy. */ () => {
      if (phase !== PHASE.idle) controller.cancel(false);
    },
    [phase, controller],
  );
  useEffect(
    /** Returns preview lifecycle cleanup. */ () =>
      /** Releases pending editor work. */ () =>
        controller.dispose(),
    [controller],
  );
  const binding: RenameBinding = {
    snapshot,
    open: controller.open,
    change: controller.change,
    save: controller.save,
    cancel: controller.cancel,
  };
  return {
    binding,
    macros: macros.map(
      /** @param macro Scenario row. @returns Its preview name override, if any. */ (
        macro,
      ) => ({ ...macro, name: names[macro.id] ?? macro.name }),
    ),
  };
}
