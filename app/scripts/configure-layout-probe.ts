const SELECTOR = {
  frames: 'iframe',
  footer: 'footer',
  status: 'footer [role="status"]',
  buttons: 'footer button',
  fields: '.configure-fields',
} as const;
const GEOMETRY_TOLERANCE = 0.5;
const EMPTY_STATUS = '';
const PENDING_ATTRIBUTE = 'true';

/** @param document Rendered fixture document. @param selector Required production surface. @returns Its mounted element; missing surfaces fail the browser probe. */
function requiredElement(document: Document, selector: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`Missing Configure surface: ${selector}`);
  return element;
}

/** @param text Painted text fragment. @param button Action control bounds. @returns Horizontal overlap only when both axes intersect beyond rounding tolerance. */
function overlapWidth(text: DOMRect, button: DOMRect): number {
  const height =
    Math.min(text.bottom, button.bottom) - Math.max(text.top, button.top);
  if (height <= GEOMETRY_TOLERANCE) return 0;
  return Math.max(
    0,
    Math.min(text.right, button.right) - Math.max(text.left, button.left),
  );
}

/** @param frame Real form fixture with a bounded logical viewport. @param pendingStatus Expected source-owned progress message. @returns Missing, clipped, or overlapping footer content diagnostics, using text ranges rather than paragraph boxes. */
function checkFrame(frame: HTMLIFrameElement, pendingStatus: string): string[] {
  const document = frame.contentDocument;
  if (!document) throw new Error('Configure fixture document did not load');
  const footer = requiredElement(document, SELECTOR.footer);
  const status = requiredElement(document, SELECTOR.status);
  const fields = requiredElement(document, SELECTOR.fields);
  const expected =
    frame.dataset.pending === PENDING_ATTRIBUTE ? pendingStatus : EMPTY_STATUS;
  const failures: string[] = [];
  if (status.textContent !== expected)
    failures.push(`${frame.title}: incorrect progress text`);
  if (
    footer.getBoundingClientRect().bottom >
    frame.clientHeight + GEOMETRY_TOLERANCE
  )
    failures.push(`${frame.title}: footer extends below the viewport`);
  if (fields.clientHeight <= 0)
    failures.push(`${frame.title}: field scroller collapsed`);
  failures.push(...checkStatusText(frame, status));
  return failures;
}

/** @param frame Bounded viewport. @param status Real progress paragraph. @returns Painted text clipping and action-overlap diagnostics, including glyphs that overflow their paragraph's box. */
function checkStatusText(
  frame: HTMLIFrameElement,
  status: HTMLElement,
): string[] {
  const document = status.ownerDocument;
  const range = document.createRange();
  range.selectNodeContents(status);
  const failures: string[] = [];
  for (const text of range.getClientRects()) {
    if (text.left < 0 || text.right > frame.clientWidth + GEOMETRY_TOLERANCE)
      failures.push(
        `${frame.title}: progress text extends outside the viewport`,
      );
    for (const button of document.querySelectorAll<HTMLElement>(
      SELECTOR.buttons,
    )) {
      const overlap = overlapWidth(text, button.getBoundingClientRect());
      if (overlap > GEOMETRY_TOLERANCE)
        failures.push(
          `${frame.title}: progress text overlaps ${button.textContent} by ${overlap.toFixed(1)}px`,
        );
    }
  }
  return failures;
}

/** @param pendingStatus Expected progress text from the Configure contract. @returns Case count and exact text-overlap/viewport failures after all fixture documents finish loading. */
export function checkConfigureFooters(pendingStatus: string): {
  cases: number;
  failures: string[];
} {
  const frames = [
    ...document.querySelectorAll<HTMLIFrameElement>(SELECTOR.frames),
  ];
  return {
    cases: frames.length,
    failures: frames.flatMap(
      /** @param frame One viewport/text-size/state combination. @returns Its rendered footer failures. */
      (frame) => checkFrame(frame, pendingStatus),
    ),
  };
}
