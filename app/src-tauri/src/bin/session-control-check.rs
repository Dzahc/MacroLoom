//! Bounded development-only real-registration, visible conflict, and focused-window session checks.

/// Runs the isolated check against Vite; requires free F8/F9 and sends no keyboard input to other applications.
fn main() {
    macroloom_lib::check_session_controls();
}
