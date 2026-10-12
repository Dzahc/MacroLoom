//! Compile-time routing metadata; library builds now support native recording while temporary replay remains a debug control.
use serde::Serialize;
const LIBRARY_DISABLED: &str = "Saved-library actions are unavailable in the input prototype";
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
/// Selects an explicit debug prototype route; release builds always use the real library.
pub struct AppMode {
    pub input_prototype: bool,
}
impl AppMode {
    /// Returns compile-time routing metadata without starting native input or shortcuts.
    pub fn current() -> Self {
        Self {
            input_prototype: cfg!(all(debug_assertions, feature = "input-prototype")),
        }
    }
    /// Rejects saved-library operations in the separately routed temporary input prototype.
    pub fn require_library(self) -> Result<(), String> {
        if self.input_prototype {
            Err(LIBRARY_DISABLED.into())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Saved-library mutations are allowed in the default route and rejected in the explicit prototype route.
    fn saved_library_mutation_requires_library_mode() {
        assert!(AppMode {
            input_prototype: false
        }
        .require_library()
        .is_ok());
        assert_eq!(
            AppMode {
                input_prototype: true
            }
            .require_library(),
            Err(LIBRARY_DISABLED.into())
        );
    }
}
