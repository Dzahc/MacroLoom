import { ACTION, type ActionName } from './library-model';

const PATH = {
  play: 'M8 5v14l11-7z',
  stop: 'M6 6h12v12H6z',
  configure:
    'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1Z',
  delete: 'M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7',
  close: 'm6 6 12 12M18 6 6 18',
  success: 'm5 12 4 4L19 6',
  failure: 'M12 7v6m0 4v.1M12 3 2 21h20Z',
} as const;

/**
 * Renders the decorative icon for a library action or notification.
 * @param props Action identifying the SVG glyph; the parent supplies its accessible name.
 * @returns An aria-hidden SVG with no listeners or external effects.
 */
export function ActionIcon({
  action,
}: {
  action: ActionName | 'close' | 'success' | 'failure';
}) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {action === ACTION.record ? (
        <circle cx="12" cy="12" r="6" fill="currentColor" stroke="none" />
      ) : (
        <path
          d={PATH[action]}
          fill={
            action === ACTION.play || action === ACTION.stop
              ? 'currentColor'
              : 'none'
          }
        />
      )}
    </svg>
  );
}
