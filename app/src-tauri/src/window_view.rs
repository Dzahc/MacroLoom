use serde::{Deserialize, Serialize};

pub const MIN_CONTENT_WIDTH: f64 = 360.0;
const MAX_CONTENT_SIZE: f64 = 4096.0;
const INVALID_SIZE: &str = "Invalid compact content measurements";

#[derive(Clone, Copy, Debug, PartialEq)]
/// Signed physical screen coordinate; negative origins are valid on secondary monitors.
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Logical content size; only the platform adapter converts it into a physical outer frame.
pub struct ContentSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Typed UI measurement request; never accepts native handles or an arbitrary target window.
pub struct ViewRequest {
    pub compact: bool,
    pub width: f64,
    pub height: f64,
}

impl ViewRequest {
    /// Validates finite bounded logical measurements before any native mutation.
    fn size(self) -> Result<ContentSize, String> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < MIN_CONTENT_WIDTH
            || self.width > MAX_CONTENT_SIZE
            || self.height < 1.0
            || self.height > MAX_CONTENT_SIZE
        {
            return Err(INVALID_SIZE.into());
        }
        Ok(ContentSize {
            width: self.width,
            height: self.height,
        })
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Actual mode after native work; errors are timed-toast data, never persistent recovery UI.
pub struct ViewResult {
    pub compact: bool,
    pub error: Option<String>,
}

/// Platform seam for capturing, moving, compacting and restoring this application's main window.
pub trait WindowAdapter {
    /// Owned full placement, including geometry and native restore state.
    type Saved: Clone;
    /// Captures the full placement before mutation; failures leave native state unchanged.
    fn save(&self) -> Result<Self::Saved, String>;
    /// Returns the saved full upper-left physical screen coordinate.
    fn saved_position(&self, saved: &Self::Saved) -> Position;
    /// Reads actual current bounds so movement is retained across separate sessions.
    fn position(&self) -> Result<Position, String>;
    /// Fits content, clamps to the applicable work area, and makes the window topmost without activating it.
    fn enter(&mut self, position: Position, size: ContentSize) -> Result<(), String>;
    /// Restores full geometry and styles without activating the window; errors retain the saved placement.
    fn restore(&mut self, saved: &Self::Saved) -> Result<(), String>;
    /// Re-fits a moved compact frame to its work area and current DPI; default adapters use the existing entry boundary.
    fn fit(&mut self, size: ContentSize) -> Result<(), String> {
        let position = self.position()?;
        self.enter(position, size)
    }
}

/// Owns a single saved full placement and independently remembers the movable compact position.
pub struct WindowView<A: WindowAdapter> {
    adapter: A,
    original: Option<A::Saved>,
    remembered: Option<Position>,
    size: Option<ContentSize>,
}

impl<A: WindowAdapter> WindowView<A> {
    /// Creates full mode without reading the desktop or assuming its monitor layout.
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            original: None,
            remembered: None,
            size: None,
        }
    }

    /// Applies a validated view change and reports actual retained mode after success or rollback failure.
    pub fn update(&mut self, request: ViewRequest) -> ViewResult {
        let error = self.apply(request).err();
        ViewResult {
            compact: self.original.is_some(),
            error,
        }
    }

    /// Returns actual retained mode without changing geometry, useful when a stale status request is rejected.
    pub fn current(&self) -> ViewResult {
        ViewResult {
            compact: self.original.is_some(),
            error: None,
        }
    }

    /// Re-fits only an active compact view after native movement/DPI changes, preserving the full placement.
    pub fn refresh(&mut self) -> ViewResult {
        let error = match (self.original.as_ref(), self.size) {
            (Some(_), Some(size)) => self.adapter.fit(size).err(),
            _ => None,
        };
        ViewResult {
            compact: self.original.is_some(),
            error,
        }
    }

    /// Validates the command then changes layout; equal active measurements do no native work.
    fn apply(&mut self, request: ViewRequest) -> Result<(), String> {
        let size = request.size()?;
        if !request.compact {
            return self.restore();
        }
        if self.original.is_some() && self.size == Some(size) {
            return Ok(());
        }
        let position = self.prepare()?;
        if let Err(error) = self.adapter.enter(position, size) {
            return self.rollback(error);
        }
        self.size = Some(size);
        Ok(())
    }

    /// Saves full placement once; remeasurement uses the current moved compact position.
    fn prepare(&mut self) -> Result<Position, String> {
        if self.original.is_some() {
            return self.adapter.position();
        }
        let saved = self.adapter.save()?;
        let position = self
            .remembered
            .unwrap_or_else(|| self.adapter.saved_position(&saved));
        self.original = Some(saved);
        Ok(position)
    }

    /// Attempts entry rollback and preserves saved data on failure rather than losing the restoration path.
    fn rollback(&mut self, error: String) -> Result<(), String> {
        match self.restore() {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error}; rollback failed: {rollback}")),
        }
    }

    /// Restores full placement; remembers actual compact position only after native restoration succeeds.
    fn restore(&mut self) -> Result<(), String> {
        let Some(saved) = self.original.clone() else {
            return Ok(());
        };
        let position = self.adapter.position();
        self.adapter.restore(&saved)?;
        self.original = None;
        self.size = None;
        match position {
            Ok(position) => {
                self.remembered = Some(position);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: Position = Position { x: -800, y: 120 };
    const MOVED: Position = Position { x: -600, y: 200 };
    const HEIGHT: f64 = 156.0;
    const ENTRY_FAILURE: &str = "Entry failed after a partial native change";
    const RESTORE_FAILURE: &str = "Restore failed";

    #[derive(Clone, Copy, PartialEq)]
    /// Native boundary faults distinguish partial entry from failed restoration.
    enum Fault {
        Entry,
        Restore,
    }

    /// Models native saved-placement and movement at the platform boundary without a desktop window.
    struct Desktop {
        position: Position,
        restored: Option<Position>,
        fault: Option<Fault>,
    }
    impl WindowAdapter for Desktop {
        type Saved = Position;
        /// Returns the desktop's current full placement.
        fn save(&self) -> Result<Position, String> {
            Ok(self.position)
        }
        /// Returns the upper-left screen coordinate of a saved placement.
        fn saved_position(&self, saved: &Position) -> Position {
            *saved
        }
        /// Returns the current moved compact position.
        fn position(&self) -> Result<Position, String> {
            Ok(self.position)
        }
        /// Applies a compact position; content sizing belongs to the native adapter.
        fn enter(&mut self, position: Position, _size: ContentSize) -> Result<(), String> {
            if self.fault == Some(Fault::Entry) {
                self.position = MOVED;
                return Err(ENTRY_FAILURE.into());
            }
            self.position = position;
            Ok(())
        }
        /// Restores the saved full position and records the visible native result.
        fn restore(&mut self, saved: &Position) -> Result<(), String> {
            if self.fault == Some(Fault::Restore) {
                return Err(RESTORE_FAILURE.into());
            }
            self.position = *saved;
            self.restored = Some(*saved);
            Ok(())
        }
    }

    #[test]
    /// Compact movement is remembered independently of the preserved full placement.
    fn restores_full_placement_and_remembers_compact_movement() {
        let mut window = WindowView::new(Desktop {
            position: FULL,
            restored: None,
            fault: None,
        });
        let request = ViewRequest {
            compact: true,
            width: MIN_CONTENT_WIDTH,
            height: HEIGHT,
        };
        assert!(window.update(request).error.is_none());
        window.adapter.position = MOVED;
        assert!(
            !window
                .update(ViewRequest {
                    compact: false,
                    ..request
                })
                .compact
        );
        assert_eq!(window.adapter.restored, Some(FULL));
        assert!(window.update(request).compact);
        assert_eq!(window.adapter.position, MOVED);
    }

    #[test]
    /// Partial native entry is rolled back to full placement and reported as a transient error result.
    fn failed_entry_rolls_back_to_the_saved_full_window() {
        let mut window = WindowView::new(Desktop {
            position: FULL,
            restored: None,
            fault: Some(Fault::Entry),
        });
        let result = window.update(ViewRequest {
            compact: true,
            width: MIN_CONTENT_WIDTH,
            height: HEIGHT,
        });
        assert!(!result.compact);
        assert_eq!(result.error.as_deref(), Some(ENTRY_FAILURE));
        assert_eq!(window.adapter.position, FULL);
    }

    #[test]
    /// Failed restoration retains the native compact position and full placement for a later ordinary view transition.
    fn failed_restoration_retains_actual_compact_mode_and_saved_full_placement() {
        let mut window = WindowView::new(Desktop {
            position: FULL,
            restored: None,
            fault: None,
        });
        let request = ViewRequest {
            compact: true,
            width: MIN_CONTENT_WIDTH,
            height: HEIGHT,
        };
        assert!(window.update(request).compact);
        window.adapter.position = MOVED;
        window.adapter.fault = Some(Fault::Restore);
        let result = window.update(ViewRequest {
            compact: false,
            ..request
        });
        assert!(result.compact);
        assert_eq!(result.error.as_deref(), Some(RESTORE_FAILURE));
        assert_eq!(window.adapter.position, MOVED);
        window.adapter.fault = None;
        assert!(
            !window
                .update(ViewRequest {
                    compact: false,
                    ..request
                })
                .compact
        );
        assert_eq!(window.adapter.position, FULL);
    }

    #[test]
    /// Nonfinite frontend measurements are rejected before any native placement is saved or changed.
    fn invalid_external_measurements_cannot_change_the_window() {
        let mut window = WindowView::new(Desktop {
            position: FULL,
            restored: None,
            fault: None,
        });
        let result = window.update(ViewRequest {
            compact: true,
            width: f64::NAN,
            height: HEIGHT,
        });
        assert!(!result.compact);
        assert_eq!(result.error.as_deref(), Some(INVALID_SIZE));
        assert_eq!(window.adapter.position, FULL);
        assert_eq!(window.adapter.restored, None);
    }
}
