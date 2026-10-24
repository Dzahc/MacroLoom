import assert from 'node:assert/strict';
import { test } from 'node:test';
import { IDLE_PRESENTATION, PLAYBACK_STAGE } from '../src/compact-contract.ts';
import { sessionBanner } from '../src/compact-presentation.ts';
import { PHASE } from '../src/library-model.ts';

const POSITIVE_REMAINDER_MS = 49;
const EXPECTED_COUNTDOWN = 'Next run in 0.1 s · 00:00';
const MACRO_NAME = 'Fill form';
const ELAPSED_MS = 18_000;
const RUN = 2;
const TOTAL_RUNS = 5;
const START_TITLE = 'Starting “Fill form”';
const FINISH_TITLE = 'Finishing run 2/5';
const ELAPSED_TEXT = '00:18';

/** A positive interval never displays an expired countdown; rendering does not alter its backend deadline. */
void test('repeat countdown rounds upward while waiting', () => {
  const banner = sessionBanner({
    macros: [],
    selectedId: null,
    message: '',
    phase: PHASE.interval,
    session: { ...IDLE_PRESENTATION, remainingMs: POSITIVE_REMAINDER_MS },
  });
  assert.equal(banner.detail, EXPECTED_COUNTDOWN);
});

/** Preparation and input release remain visible as session time without inventing an active run or countdown. */
void test('playing status distinguishes startup and per-run cleanup', () => {
  for (const [playbackStage, title] of [
    [PLAYBACK_STAGE.starting, START_TITLE],
    [PLAYBACK_STAGE.finishing, FINISH_TITLE],
  ] as const) {
    const banner = sessionBanner({
      macros: [],
      selectedId: null,
      message: '',
      phase: PHASE.playing,
      session: {
        ...IDLE_PRESENTATION,
        playbackStage,
        macroName: MACRO_NAME,
        elapsedMs: ELAPSED_MS,
        run: RUN,
        totalRuns: TOTAL_RUNS,
      },
    });
    assert.equal(banner.title, title);
    assert.equal(banner.detail, ELAPSED_TEXT);
  }
});
