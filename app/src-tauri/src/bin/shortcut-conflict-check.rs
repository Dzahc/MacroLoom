//! Development-only manual conflict fixture; reserve Record or Stop until Enter.
/// Runs the native conflict helper and returns a visible failure rather than silently failing to reserve the key.
fn main() -> Result<(), String> {
    macroloom_lib::hold_conflict_shortcut()
}
