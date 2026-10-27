//! Deterministic physical-frame policy shared by Configure opening and display recovery.

use std::cmp::Reverse;

const CENTER_DIVISOR: i64 = 2;
const PLACEMENT_PASSES: usize = 2;
pub const PLACEMENT_FAILED: &str = "Could not place Configure visibly. Reconnect a display or enlarge its usable area, then Retry.";

#[derive(Clone, Copy, Debug)]
/// Positive decorated physical size, converted by the native boundary at the destination DPI.
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Signed physical outer bounds; right and bottom edges are exclusive.
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    /// Returns whether both dimensions are positive, without overflowing signed screen coordinates.
    pub fn valid(self) -> bool {
        self.left < self.right && self.top < self.bottom
    }

    /// Returns intersection area in physical pixels, including layouts spanning the full i32 coordinate range.
    fn overlap(self, other: Self) -> u64 {
        let width =
            (i64::from(self.right.min(other.right)) - i64::from(self.left.max(other.left))).max(0);
        let height =
            (i64::from(self.bottom.min(other.bottom)) - i64::from(self.top.max(other.top))).max(0);
        width as u64 * height as u64
    }

    /// Returns squared distance from a physical pixel to this display, with zero only for contained pixels.
    fn corner_distance(self, x: i32, y: i32) -> u128 {
        let dx = i64::from(x) - i64::from(x.clamp(self.left, self.right - 1));
        let dy = i64::from(y) - i64::from(y.clamp(self.top, self.bottom - 1));
        (i128::from(dx).pow(2) + i128::from(dy).pow(2)) as u128
    }

    /// Returns squared separation of two outer frames; overlap and touching edges have zero separation.
    fn distance(self, other: Self) -> u128 {
        let dx = (i64::from(self.left) - i64::from(other.right))
            .max(i64::from(other.left) - i64::from(self.right))
            .max(0);
        let dy = (i64::from(self.top) - i64::from(other.bottom))
            .max(i64::from(other.top) - i64::from(self.bottom))
            .max(0);
        (i128::from(dx).pow(2) + i128::from(dy).pow(2)) as u128
    }
}

#[derive(Clone, Copy, Debug)]
/// One currently available display's full bounds, usable bounds, and primary designation.
pub struct Display {
    pub bounds: Rect,
    pub work: Rect,
    pub primary: bool,
}

/// Native boundary for fresh display discovery, destination-DPI frame conversion, and verified nonactivating placement.
pub trait PlacementDesktop {
    /// Returns the owner's current physical outer bounds, or None when unavailable.
    fn owner(&self) -> Option<Rect>;
    /// Returns current full/work display geometry; errors permit a fresh discovery retry.
    fn displays(&self) -> Result<Vec<Display>, String>;
    /// Returns preferred/minimum decorated physical sizes for this destination, or a conversion error.
    fn sizes(&self, display: &Display) -> Result<(Size, Size), String>;
    /// Applies and verifies the frame on the current destination without activating another application.
    fn apply(&self, frame: Rect, display: &Display) -> Result<(), String>;
}

/// Tries the requested display, nearest display, and primary, refreshing discovery once after complete failure.
/// Success requires native verification; no native failure can produce an invisible successful opening.
pub fn place(desktop: &impl PlacementDesktop) -> Result<Rect, String> {
    for _ in 0..PLACEMENT_PASSES {
        let displays = match desktop.displays() {
            Ok(displays) => displays,
            Err(_) => continue,
        };
        let owner = desktop.owner().filter(|owner| owner.valid());
        for index in destinations(owner, &displays) {
            if let Ok(frame) = place_on_display(desktop, owner, &displays[index]) {
                return Ok(frame);
            }
        }
    }
    Err(PLACEMENT_FAILED.into())
}

/// Converts and fits one candidate before native mutation, propagating failures to the fallback policy.
fn place_on_display(
    desktop: &impl PlacementDesktop,
    owner: Option<Rect>,
    display: &Display,
) -> Result<Rect, String> {
    let (desired, minimum) = desktop.sizes(display)?;
    let frame = fit(owner, display.work, desired, minimum)?;
    desktop.apply(frame, display)?;
    Ok(frame)
}

/// Returns candidate indices with largest overlap first, resolving ties by the owner's top-left corner and stable geometry.
pub fn destinations(owner: Option<Rect>, displays: &[Display]) -> Vec<usize> {
    let mut indices: Vec<_> = (0..displays.len()).collect();
    indices.retain(|&index| displays[index].bounds.valid() && displays[index].work.valid());
    indices.sort_by_key(|&index| selection_key(owner, displays[index]));
    let primary = indices
        .iter()
        .copied()
        .find(|&index| displays[index].primary);
    let nearest = indices.iter().copied().min_by_key(|&index| {
        let bounds = displays[index].bounds;
        (
            owner.map_or(0, |owner| bounds.distance(owner)),
            Reverse(owner.map_or(0, |owner| bounds.overlap(owner))),
            bounds.left,
            bounds.top,
        )
    });
    let preferred = if owner.is_some() {
        indices.first().copied()
    } else {
        primary.or(nearest)
    };
    let mut result = Vec::new();
    for index in [preferred, nearest.filter(|_| owner.is_some()), primary]
        .into_iter()
        .flatten()
    {
        if !result.contains(&index) {
            result.push(index);
        }
    }
    result
}

/// Orders valid displays by overlap, owner-corner preference, and stable geometry; offscreen owners use frame distance.
fn selection_key(
    owner: Option<Rect>,
    display: Display,
) -> (Reverse<u64>, u128, i32, i32, i32, i32) {
    let bounds = display.bounds;
    let (area, distance) = owner.map_or((0, 0), |owner| {
        let area = owner.overlap(bounds);
        let distance = if area == 0 {
            bounds.distance(owner)
        } else {
            bounds.corner_distance(owner.left, owner.top)
        };
        (area, distance)
    });
    (
        Reverse(area),
        distance,
        bounds.left,
        bounds.top,
        bounds.right,
        bounds.bottom,
    )
}

/// Centers a decorated frame over the owner (or work area), shrinking to fit while preserving the decorated minimum.
/// Invalid or undersized work areas return an actionable opening/recovery error without changing native state.
pub fn fit(owner: Option<Rect>, work: Rect, desired: Size, minimum: Size) -> Result<Rect, String> {
    if !work.valid() || minimum.width <= 0 || minimum.height <= 0 {
        return Err(PLACEMENT_FAILED.into());
    }
    let width = i64::from(desired.width).min(i64::from(work.right) - i64::from(work.left));
    let height = i64::from(desired.height).min(i64::from(work.bottom) - i64::from(work.top));
    if width < i64::from(minimum.width) || height < i64::from(minimum.height) {
        return Err(PLACEMENT_FAILED.into());
    }
    let center = owner.filter(|owner| owner.valid()).unwrap_or(work);
    let left = ((i64::from(center.left) + i64::from(center.right) - width) / CENTER_DIVISOR)
        .clamp(i64::from(work.left), i64::from(work.right) - width);
    let top = ((i64::from(center.top) + i64::from(center.bottom) - height) / CENTER_DIVISOR)
        .clamp(i64::from(work.top), i64::from(work.bottom) - height);
    // Every edge lies within the validated i32 work area; width/height are positive and bounded by desired i32 sizes.
    Ok(Rect {
        left: left as i32,
        top: top as i32,
        right: (left + width) as i32,
        bottom: (top + height) as i32,
    })
}

/// Returns whether the entire existing frame is covered by the union of usable display areas.
/// Unlike initial placement, a healthy user-positioned editor may span adjacent displays without being recentered.
pub fn reachable(frame: Rect, displays: &[Display]) -> bool {
    if !frame.valid() {
        return false;
    }
    let mut uncovered = vec![frame];
    for display in displays {
        uncovered = uncovered
            .into_iter()
            .flat_map(|piece| subtract_work_area(piece, display.work))
            .collect();
        if uncovered.is_empty() {
            return true;
        }
    }
    false
}

/// Subtracts one usable area from an uncovered rectangle into disjoint valid pieces; invalid work areas cover nothing.
fn subtract_work_area(frame: Rect, work: Rect) -> Vec<Rect> {
    let overlap = Rect {
        left: frame.left.max(work.left),
        top: frame.top.max(work.top),
        right: frame.right.min(work.right),
        bottom: frame.bottom.min(work.bottom),
    };
    if !overlap.valid() {
        return vec![frame];
    }
    [
        Rect {
            right: overlap.left,
            ..frame
        },
        Rect {
            left: overlap.right,
            ..frame
        },
        Rect {
            left: overlap.left,
            right: overlap.right,
            bottom: overlap.top,
            ..frame
        },
        Rect {
            left: overlap.left,
            right: overlap.right,
            top: overlap.bottom,
            ..frame
        },
    ]
    .into_iter()
    .filter(|piece| piece.valid())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const PRIMARY: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    const SECONDARY: Rect = Rect {
        left: -1920,
        top: 0,
        right: 0,
        bottom: 1080,
    };
    const OWNER: Rect = Rect {
        left: -900,
        top: 100,
        right: 900,
        bottom: 700,
    };
    const PRIMARY_INDEX: usize = 0;
    const SECONDARY_INDEX: usize = 1;

    /// Supplies two displays in deliberately primary-first enumeration order.
    fn displays() -> Vec<Display> {
        vec![
            Display {
                bounds: PRIMARY,
                work: PRIMARY,
                primary: true,
            },
            Display {
                bounds: SECONDARY,
                work: SECONDARY,
                primary: false,
            },
        ]
    }

    #[test]
    /// Equal overlap chooses the owner's top-left display, independently of enumeration order.
    fn equal_overlap_uses_top_left_corner() {
        assert_eq!(
            destinations(Some(OWNER), &displays())[PRIMARY_INDEX],
            SECONDARY_INDEX
        );
        let mut reversed = displays();
        reversed.reverse();
        assert_eq!(
            destinations(Some(OWNER), &reversed)[PRIMARY_INDEX],
            PRIMARY_INDEX
        );
    }

    #[test]
    /// Every opening follows the owner; missing bounds prefer primary and offscreen bounds prefer the nearest frame.
    fn fresh_owner_bounds_and_fallbacks_select_current_displays() {
        const OFFSCREEN: Rect = Rect {
            left: -2200,
            top: -200,
            right: -2000,
            bottom: -100,
        };
        assert_eq!(
            destinations(Some(SECONDARY), &displays()),
            vec![SECONDARY_INDEX, PRIMARY_INDEX]
        );
        assert_eq!(
            destinations(Some(PRIMARY), &displays()),
            vec![PRIMARY_INDEX]
        );
        let mut reversed = displays();
        reversed.reverse();
        assert_eq!(destinations(None, &reversed), vec![SECONDARY_INDEX]);
        assert_eq!(
            destinations(Some(OFFSCREEN), &displays()),
            vec![SECONDARY_INDEX, PRIMARY_INDEX]
        );
    }

    #[test]
    /// Signed centering respects the usable area and refuses a frame smaller than its decorated minimum.
    fn centers_then_shrinks_and_clamps_the_decorated_frame() {
        const DESIRED: Size = Size {
            width: 500,
            height: 660,
        };
        const MINIMUM: Size = Size {
            width: 380,
            height: 340,
        };
        const SMALL_WORK: Rect = Rect {
            left: -1920,
            top: 40,
            right: -1440,
            bottom: 600,
        };
        const EXPECTED: Rect = Rect {
            left: -1920,
            top: 40,
            right: -1440,
            bottom: 600,
        };
        const TOO_SMALL: Rect = Rect {
            right: -1600,
            ..SMALL_WORK
        };
        assert_eq!(
            fit(Some(OWNER), SMALL_WORK, DESIRED, MINIMUM).unwrap(),
            EXPECTED
        );
        assert!(fit(Some(OWNER), TOO_SMALL, DESIRED, MINIMUM).is_err());
        assert_eq!(
            fit(None, PRIMARY, DESIRED, MINIMUM).unwrap(),
            Rect {
                left: 710,
                top: 210,
                right: 1210,
                bottom: 870
            }
        );
    }

    /// Native placement fixture records actual attempted frames and rejects the missing secondary display.
    struct Desktop {
        attempted: RefCell<Vec<Rect>>,
        primary_available: bool,
    }
    impl PlacementDesktop for Desktop {
        /// Returns fresh owner geometry for this opening.
        fn owner(&self) -> Option<Rect> {
            Some(SECONDARY)
        }
        /// Supplies the current native display inventory.
        fn displays(&self) -> Result<Vec<Display>, String> {
            Ok(displays())
        }
        /// Supplies decorated default/minimum sizes already converted by the native adapter.
        fn sizes(&self, _: &Display) -> Result<(Size, Size), String> {
            const DESIRED: Size = Size {
                width: 500,
                height: 660,
            };
            const MINIMUM: Size = Size {
                width: 380,
                height: 340,
            };
            Ok((DESIRED, MINIMUM))
        }
        /// Simulates a disconnected secondary while only acknowledging visible primary placement.
        fn apply(&self, frame: Rect, _: &Display) -> Result<(), String> {
            self.attempted.borrow_mut().push(frame);
            if frame.left < PRIMARY.left || !self.primary_available {
                Err(PLACEMENT_FAILED.into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    /// An unavailable preferred destination falls back to a verified primary placement without claiming invisible success.
    fn failed_secondary_placement_tries_primary() {
        let desktop = Desktop {
            attempted: RefCell::new(Vec::new()),
            primary_available: true,
        };
        let frame = place(&desktop).unwrap();
        assert!(frame.left >= PRIMARY.left);
        assert_eq!(desktop.attempted.borrow().len(), displays().len());
    }

    #[test]
    /// No successful native placement means an actionable error after bounded rediscovery, never invisible success.
    fn all_native_destinations_failing_is_an_opening_error() {
        let desktop = Desktop {
            attempted: RefCell::new(Vec::new()),
            primary_available: false,
        };
        assert_eq!(place(&desktop).unwrap_err(), PLACEMENT_FAILED);
    }

    #[test]
    /// A top-left corner on a third display cannot override the largest-area rule; remaining ties use stable geometry.
    fn minority_corner_and_remaining_ties_are_deterministic() {
        const LEFT: Rect = Rect {
            left: 0,
            top: 100,
            right: 500,
            bottom: 600,
        };
        const RIGHT: Rect = Rect {
            left: 500,
            top: 100,
            right: 1000,
            bottom: 600,
        };
        const ABOVE: Rect = Rect {
            left: 0,
            top: 0,
            right: 1000,
            bottom: 100,
        };
        const SPANNING: Rect = Rect {
            left: 0,
            top: 0,
            right: 1000,
            bottom: 600,
        };
        let displays = [
            Display {
                bounds: RIGHT,
                work: RIGHT,
                primary: true,
            },
            Display {
                bounds: ABOVE,
                work: ABOVE,
                primary: false,
            },
            Display {
                bounds: LEFT,
                work: LEFT,
                primary: false,
            },
        ];
        const LEFT_INDEX: usize = 2;
        assert_eq!(
            destinations(Some(SPANNING), &displays)[PRIMARY_INDEX],
            LEFT_INDEX
        );
    }

    #[test]
    /// Destination-scaled physical frames keep usable top/taskbar edges and never silently reduce the logical minimum.
    fn scaled_frames_fit_negative_work_areas() {
        const SCALED_DEFAULT: Size = Size {
            width: 744,
            height: 990,
        };
        const SCALED_MINIMUM: Size = Size {
            width: 564,
            height: 510,
        };
        const WORK: Rect = Rect {
            left: -1280,
            top: -900,
            right: 0,
            bottom: -100,
        };
        const OWNER: Rect = Rect {
            left: -1200,
            top: -850,
            right: -600,
            bottom: -370,
        };
        const EXPECTED: Rect = Rect {
            left: -1272,
            top: -900,
            right: -528,
            bottom: -100,
        };
        assert_eq!(
            fit(Some(OWNER), WORK, SCALED_DEFAULT, SCALED_MINIMUM).unwrap(),
            EXPECTED
        );
    }

    #[test]
    /// A user-positioned editor may span adjacent healthy displays; a real gap or offscreen action area requires recovery.
    fn healthy_monitor_spanning_editor_remains_reachable() {
        const SPANNING: Rect = Rect {
            left: -300,
            top: 100,
            right: 300,
            bottom: 700,
        };
        const GAPPED_PRIMARY: Rect = Rect {
            left: 100,
            ..PRIMARY
        };
        assert!(reachable(SPANNING, &displays()));
        let mut gapped = displays();
        gapped[PRIMARY_INDEX].work = GAPPED_PRIMARY;
        assert!(!reachable(SPANNING, &gapped));
        assert!(!reachable(SPANNING, &displays()[..SECONDARY_INDEX]));
    }
}
