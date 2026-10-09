import {
  CONFIGURE_LIMIT,
  REPEAT_MODE,
  SPEED_PRESETS,
  type ConfigureDraft,
  type ConfigureErrors,
  type ConfigureSnapshot,
} from './configure-contract.ts';

export type ConfigureInputs = {
  name: string;
  speed: number;
  repeatMode: string;
  totalRuns: string;
  interval: string;
};
export const CONFIGURE_ERROR = {
  name: 'Enter a name of 1–120 characters without control characters.',
  speed: 'Choose a supported playback speed.',
  mode: 'Choose a repeat mode.',
  runs: `Enter a whole number from 1 to ${CONFIGURE_LIMIT.max}.`,
  interval:
    'Enter nonnegative decimal seconds with up to three decimal places within the safe millisecond range.',
} as const;
// Unicode properties mirror Rust's char::is_whitespace / char::is_control.
const RUST_WHITESPACE = /^\p{White_Space}+|\p{White_Space}+$/gu;
const CONTROL_CHARACTERS = /\p{Cc}/u;
const INTEGER = /^\d+$/;
const SECONDS = /^(?:\d+|\d*\.\d{1,3})$/;
const ZERO = '0';
const DECIMAL_POINT = '.';
const MAX_DIGITS = String(CONFIGURE_LIMIT.max).length;

/** @param value Raw name. @returns Name trimmed using Rust's Unicode White_Space set, including NEL and excluding BOM. */
export function trimConfigureName(value: string): string {
  return value.replace(RUST_WHITESPACE, '');
}

/** @param snapshot Validated supplied properties. @returns Independent editable strings, without floating-point conversion of milliseconds. */
export function configureInputs(snapshot: ConfigureSnapshot): ConfigureInputs {
  const milliseconds = BigInt(snapshot.playback.intervalMs);
  const unit = BigInt(CONFIGURE_LIMIT.milliseconds);
  const fraction = String(milliseconds % unit)
    .padStart(CONFIGURE_LIMIT.precision, ZERO)
    .replace(/0+$/, '');
  const interval = `${milliseconds / unit}${fraction ? `${DECIMAL_POINT}${fraction}` : ''}`;
  return {
    name: snapshot.name,
    speed: snapshot.playback.speed,
    repeatMode: snapshot.playback.repeatMode,
    totalRuns: String(snapshot.playback.totalRuns),
    interval,
  };
}

/** @param digits Decimal digits. @returns An exact safe nonnegative integer or null; bounds the BigInt parser's work. */
function safeDigits(digits: string): number | null {
  const normalized = digits.replace(/^0+/, '') || ZERO;
  if (normalized.length > MAX_DIGITS) return null;
  const value = BigInt(normalized);
  return value <= BigInt(CONFIGURE_LIMIT.max) ? Number(value) : null;
}

/** @param value Raw count. @returns Positive safe integer, or null for invalid syntax, zero, or overflow. */
export function parseTotalRuns(value: string): number | null {
  if (!INTEGER.test(value)) return null;
  const count = safeDigits(value);
  return count !== null && count >= CONFIGURE_LIMIT.firstRun ? count : null;
}

/** @param value Raw decimal seconds. @returns Exact integral milliseconds, or null; exponents, signs and extra precision are rejected. */
export function parseInterval(value: string): number | null {
  if (!SECONDS.test(value)) return null;
  const [whole, fraction = ''] = value.split(DECIMAL_POINT);
  return safeDigits(
    `${whole}${fraction.padEnd(CONFIGURE_LIMIT.precision, ZERO)}`,
  );
}

/** @param input Editable values. @returns Active field errors using backend-compatible name/count/interval rules. */
export function configureErrors(input: ConfigureInputs): ConfigureErrors {
  const errors: ConfigureErrors = {};
  const name = trimConfigureName(input.name);
  const length = Array.from(name).length;
  if (!length || length > CONFIGURE_LIMIT.name || CONTROL_CHARACTERS.test(name))
    errors.name = CONFIGURE_ERROR.name;
  if (
    !SPEED_PRESETS.some(
      /** @param speed Preset. @returns Whether it matches the draft. */ (
        speed,
      ) => speed === input.speed,
    )
  )
    errors.speed = CONFIGURE_ERROR.speed;
  if (
    !Object.values(REPEAT_MODE).some(
      /** @param mode Supported mode. @returns Whether it matches the draft. */ (
        mode,
      ) => mode === input.repeatMode,
    )
  )
    errors.repeatMode = CONFIGURE_ERROR.mode;
  if (
    input.repeatMode === REPEAT_MODE.fixed &&
    parseTotalRuns(input.totalRuns) === null
  )
    errors.totalRuns = CONFIGURE_ERROR.runs;
  if (
    input.repeatMode !== REPEAT_MODE.once &&
    parseInterval(input.interval) === null
  )
    errors.interval = CONFIGURE_ERROR.interval;
  return errors;
}

/** @param input Valid active fields. @param original Trusted baseline for invalid inactive fields. @returns Complete typed properties; disabled valid values are preserved. */
export function configureDraft(
  input: ConfigureInputs,
  original: ConfigureSnapshot,
): ConfigureDraft {
  return {
    macroId: original.macroId,
    name: trimConfigureName(input.name),
    playback: {
      speed: input.speed,
      repeatMode:
        input.repeatMode as ConfigureSnapshot['playback']['repeatMode'],
      totalRuns: parseTotalRuns(input.totalRuns) ?? original.playback.totalRuns,
      intervalMs: parseInterval(input.interval) ?? original.playback.intervalMs,
    },
  };
}
