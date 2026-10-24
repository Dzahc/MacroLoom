/** Keyboard activation keys whose complete gesture must finish before playback can inject input. */
export const PLAYBACK_KEY = { enter: 'Enter', space: ' ' } as const;
type ActivationEvent = Pick<KeyboardEvent, 'key' | 'repeat' | 'preventDefault'>;

/** @param event Button key-down. @returns Nothing; prevents native early/repeated clicks for Enter and Space only. */
export function playbackKeyDown(event: ActivationEvent): void {
  if (isPlaybackKey(event.key)) event.preventDefault();
}

/** @param event Button key-up. @param activate Guarded Play callback. @returns Nothing; completes activation after release without an added timer. */
export function playbackKeyUp(
  event: ActivationEvent,
  activate: () => void,
): void {
  if (!isPlaybackKey(event.key)) return;
  event.preventDefault();
  if (!event.repeat) activate();
}

/** @param key Observed key value. @returns Whether the button's activation gesture belongs to playback. */
function isPlaybackKey(key: string): boolean {
  return key === PLAYBACK_KEY.enter || key === PLAYBACK_KEY.space;
}
