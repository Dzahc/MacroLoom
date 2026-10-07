//! Explicit development-only executable with Tauri's Windows manifest for real WebView regression checks.

/// Runs the bounded preview-load/normal-close check against Vite in development; rejects release execution.
fn main() {
    macroloom_lib::check_development_preview();
}
