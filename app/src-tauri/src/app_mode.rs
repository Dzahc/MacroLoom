use serde::Serialize;

#[cfg(all(debug_assertions, feature = "input-prototype"))]
const INPUT_PROTOTYPE_ENABLED: bool = true;
#[cfg(not(all(debug_assertions, feature = "input-prototype")))]
const INPUT_PROTOTYPE_ENABLED: bool = false;
const PROTOTYPE_DISABLED: &str = "Live input is disabled in the library preview";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
/// Serialized launch metadata; only explicit debug prototype builds may enable live input.
pub struct AppMode {
    pub input_prototype: bool,
}

impl AppMode {
    /// Returns compile-time mode metadata without starting hooks or registering shortcuts.
    /// Release builds always return library mode, even with the prototype feature selected.
    pub fn current() -> Self {
        Self {
            input_prototype: INPUT_PROTOTYPE_ENABLED,
        }
    }
    /// Checks this mode before a live input command can change session state.
    /// Returns `Ok(())` when enabled, otherwise the library-mode error with no side effects.
    pub fn require_input(self) -> Result<(), String> {
        if self.input_prototype {
            Ok(())
        } else {
            Err(PROTOTYPE_DISABLED.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "input-prototype"))]
    /// Verifies default builds reject live commands without enabling the prototype feature.
    fn default_build_cannot_start_live_input() {
        assert_eq!(
            AppMode::current().require_input(),
            Err(PROTOTYPE_DISABLED.into())
        );
    }

    #[test]
    #[cfg(all(debug_assertions, feature = "input-prototype"))]
    /// Verifies an explicitly selected debug feature permits live commands.
    fn explicit_debug_feature_enables_live_input() {
        assert_eq!(AppMode::current().require_input(), Ok(()));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    /// Verifies release builds reject live commands regardless of feature selection.
    fn release_build_cannot_start_live_input() {
        assert_eq!(
            AppMode::current().require_input(),
            Err(PROTOTYPE_DISABLED.into())
        );
    }
    #[test]
    /// Verifies constructed library metadata fails the command precondition.
    fn library_mode_rejects_live_commands() {
        let mode = AppMode {
            input_prototype: false,
        };
        assert_eq!(mode.require_input(), Err(PROTOTYPE_DISABLED.into()));
    }
    #[test]
    /// Verifies constructed prototype metadata passes the command precondition.
    fn explicitly_enabled_prototype_accepts_live_commands() {
        let mode = AppMode {
            input_prototype: true,
        };
        assert_eq!(mode.require_input(), Ok(()));
    }
}
