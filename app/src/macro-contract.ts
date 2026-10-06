import type { MacroSummary } from './library-model';

export type PlaybackProperties = Readonly<{
  speed: number;
  repeatMode: 'once' | 'fixed' | 'indefinite';
  totalRuns: number;
  intervalMs: number;
}>;
export type StoredMacroSummary = MacroSummary &
  Readonly<{ playback: PlaybackProperties }>;
export type LoadFailure = Readonly<{
  file: string;
  field: string;
  message: string;
}>;
export type BackendLibraryState = Readonly<{
  revision: number;
  loading: boolean;
  writable: boolean;
  deleting: string | null;
  macros: readonly StoredMacroSummary[];
  failures: readonly LoadFailure[];
}>;
export type NativeKey = Readonly<{
  scanCode: number;
  virtualKey: number;
  extended: boolean;
}>;
export type RecordedEvent = Readonly<{ atMs: number }> &
  (
    | Readonly<{ type: 'key_down' | 'key_up'; key: string; native: NativeKey }>
    | Readonly<{
        type: 'mouse_down' | 'mouse_up';
        button: 'left' | 'right';
        x: number;
        y: number;
      }>
    | Readonly<{ type: 'mouse_move'; x: number; y: number }>
    | Readonly<{
        type: 'mouse_wheel';
        axis: 'vertical';
        delta: number;
        x: number;
        y: number;
      }>
  );
export type MacroDocument = Readonly<{
  schemaVersion: number;
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  recording: Readonly<{
    platform: 'windows';
    coordinateSpace: 'screen_physical_pixels';
    keyboardLayout: string;
    displays: readonly Readonly<{
      x: number;
      y: number;
      width: number;
      height: number;
      scaleFactor: number;
    }>[];
  }>;
  durationMs: number;
  playback: PlaybackProperties;
  events: readonly RecordedEvent[];
}>;

export const LIBRARY_COMMAND = {
  load: 'load_library',
  snapshot: 'macro_snapshot',
  delete: 'delete_macro',
} as const;
export const LIBRARY_EVENT = 'library-state';
