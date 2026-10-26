import assert from 'node:assert/strict';
import { test } from 'node:test';
import { restoreLibraryScroll } from '../src/compact-layout.ts';

const COMPACT_OFFSET = 0;
const LIST_OFFSET = 80;
const PANEL_OFFSET = 120;

/** Full-view restoration retains the independently scrolled list and outer panel after compact layout resets their offsets. */
void test('restoration preserves both library scroll surfaces', () => {
  const list = { scrollTop: COMPACT_OFFSET };
  const owner = {
    scrollTop: COMPACT_OFFSET,
    /** @returns The still-mounted macro list, without replacing scroll-restoration logic. */
    querySelector: () => list,
  };
  restoreLibraryScroll(owner, LIST_OFFSET, PANEL_OFFSET);
  assert.equal(list.scrollTop, LIST_OFFSET);
  assert.equal(owner.scrollTop, PANEL_OFFSET);
});

/** An empty library still restores its outer panel without requiring a list node. */
void test('restoration supports an empty library and the default panel offset', () => {
  const owner = {
    scrollTop: PANEL_OFFSET,
    /** @returns No list while the first-recording empty state is mounted. */
    querySelector: () => null,
  };
  restoreLibraryScroll(owner, LIST_OFFSET);
  assert.equal(owner.scrollTop, COMPACT_OFFSET);
  restoreLibraryScroll(owner, LIST_OFFSET, PANEL_OFFSET);
  assert.equal(owner.scrollTop, PANEL_OFFSET);
});
