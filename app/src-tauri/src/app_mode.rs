use serde::Serialize;

#[cfg(all(debug_assertions, feature = "input-prototype"))]
const INPUT_PROTOTYPE_ENABLED: bool = true;
#[cfg(not(all(debug_assertions, feature = "input-prototype")))]
const INPUT_PROTOTYPE_ENABLED: bool = false;
const PROTOTYPE_DISABLED: &str = "Live input is disabled in the library preview";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMode {
    pub input_prototype: bool,
}

impl AppMode {
    pub fn current() -> Self {
        Self {
            input_prototype: INPUT_PROTOTYPE_ENABLED,
        }
    }
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
    fn default_build_cannot_start_live_input() {
        assert_eq!(
            AppMode::current().require_input(),
            Err(PROTOTYPE_DISABLED.into())
        );
    }

    #[test]
    #[cfg(all(debug_assertions, feature = "input-prototype"))]
    fn explicit_debug_feature_enables_live_input() {
        assert_eq!(AppMode::current().require_input(), Ok(()));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_build_cannot_start_live_input() {
        assert_eq!(
            AppMode::current().require_input(),
            Err(PROTOTYPE_DISABLED.into())
        );
    }
    #[test]
    fn library_mode_rejects_live_commands() {
        let mode = AppMode {
            input_prototype: false,
        };
        assert_eq!(mode.require_input(), Err(PROTOTYPE_DISABLED.into()));
    }
    #[test]
    fn explicitly_enabled_prototype_accepts_live_commands() {
        let mode = AppMode {
            input_prototype: true,
        };
        assert_eq!(mode.require_input(), Ok(()));
    }
}
