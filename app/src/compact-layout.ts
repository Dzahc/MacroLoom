import { COMPACT, type WindowViewRequest } from './compact-contract';

const TOOLBAR_SELECTOR = '.library-toolbar';
const BANNER_SELECTOR = '.library-banner';
const LIST_SELECTOR = '.macro-list';
const TOOLBAR_BOTTOM = '--compact-toolbar-bottom';

/** @param element CSS length string. @returns Its finite pixel value, or zero for nonnumeric computed values. */
function pixels(element: string): number {
  return Number.parseFloat(element) || 0;
}

/** @param owner Mounted library root. @param compact Requested view. @returns Content-only logical dimensions, independent of hidden-list space. */
export function measureCompact(
  owner: HTMLElement,
  compact: boolean,
): WindowViewRequest {
  const toolbar = owner.querySelector<HTMLElement>(TOOLBAR_SELECTOR);
  const banner = owner.querySelector<HTMLElement>(BANNER_SELECTOR);
  const style = getComputedStyle(owner);
  const horizontal = pixels(style.paddingLeft) + pixels(style.paddingRight);
  const vertical = pixels(style.paddingTop) + pixels(style.paddingBottom);
  const toolbarHeight = toolbar?.offsetHeight ?? 0;
  const bannerHeight = banner?.offsetHeight ?? 0;
  const gap = pixels(style.rowGap);
  owner.style.setProperty(
    TOOLBAR_BOTTOM,
    `${pixels(style.paddingTop) + toolbarHeight + gap}px`,
  );
  return {
    compact,
    width: Math.ceil(
      Math.max(COMPACT.minimumWidth, (toolbar?.scrollWidth ?? 0) + horizontal),
    ),
    height: Math.ceil(vertical + toolbarHeight + gap + bannerHeight),
  };
}

/** @param owner Mounted library root. @param compact Desired view. @param receive Native-size consumer. @returns Cleanup releasing all content observers. */
export function observeCompactLayout(
  owner: HTMLElement,
  compact: boolean,
  receive: (request: WindowViewRequest) => void,
): () => void {
  /** Publishes measured toolbar/banner dimensions after content or text scale changes. */
  function update() {
    receive(measureCompact(owner, compact));
  }
  update();
  const observer = new ResizeObserver(update);
  observer.observe(owner);
  const toolbar = owner.querySelector(TOOLBAR_SELECTOR);
  const banner = owner.querySelector(BANNER_SELECTOR);
  if (toolbar) observer.observe(toolbar);
  if (banner) observer.observe(banner);
  /** Disconnects measurements after the view changes or unmounts. */
  return () => observer.disconnect();
}

/** @param owner Mounted library root. @param position Preserved list scroll offset. @returns Nothing; restoration changes no keyboard/native focus. */
export function restoreLibraryScroll(
  owner: HTMLElement,
  position: number,
): void {
  const list = owner.querySelector<HTMLElement>(LIST_SELECTOR);
  if (list) list.scrollTop = position;
}
